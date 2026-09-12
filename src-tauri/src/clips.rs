//! Quick Clips (ticket 78): the machine-local, hand-authored plain-text list
//! with one-click re-copying — storage, validation, and ordering.
//!
//! Image Clips (ticket 178) extend the same list without touching text-clip
//! behavior: an image-only Clip is a row in `clips` with blank content plus
//! exactly one row in `clip_images` (PNG or JPEG bytes, 5 MB cap, one image
//! per Clip v1). Lists carry the image metadata + bytes; dedup/collision
//! rules stay text-only (image rows carry blank content, and validated text
//! is never blank, so the two kinds never collide).
//!
//! Glossary (docs/CONTEXT.md): a **Clip** is a machine-local piece of plain
//! text stored for one-click re-copying; authored by hand (pasted into the
//! add dialog), ordered by the user, never captured in the background. All
//! editing happens in the main app's Quick Clips page; the Quick Launch
//! window's third tab is read-only (ticket 79). Machine-local like Quick
//! Actions — never part of Presets, Plan, or Preset exports.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

/// The editable shape of a Clip, as the frontend sends it. The stored record
/// ([`Clip`]) adds the id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipInput {
    /// Display name; an untitled clip stores the empty string and its
    /// content's first line stands in wherever the name is shown — the list
    /// stays readable without forcing the user to invent names.
    pub name: String,
    /// The plain text a copy puts back on the clipboard.
    pub content: String,
    /// Whether the Quick Launch dock lists this clip. The main-app page always
    /// sees every clip; the dock filters on this flag. Missing in legacy
    /// backups means visible.
    #[serde(default = "default_show_in_dock")]
    pub show_in_dock: bool,
    /// The attached image for an image-only Clip (ticket 178). `None` is a
    /// text Clip — unchanged behavior. `Some` means an image-only Clip whose
    /// `content` stays blank (mixed text+image Clips are explicitly not
    /// built). Skipped in serialization when absent so text-clip backups
    /// keep their exact shape; defaulted on read so text-only backups still
    /// parse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ClipImage>,
}

/// One image attached to a Clip (ticket 178): the normalized bytes plus the
/// metadata lists and details render without re-decoding. `hash` is the
/// FNV-1a identity over the raw bytes — the backup merge key, with the name
/// display-only. Pure data: no new runtime dependency (the bytes hash is
/// hand-rolled so `Cargo.toml` stays untouched).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipImage {
    /// Canonical mime: `image/png` or `image/jpeg` (sniffed, never trusted
    /// from the sender).
    pub mime: String,
    /// The normalized raw bytes (exactly what validation accepted).
    pub bytes_base64: String,
    /// Decoded pixel dimensions at ingest (PNG header / JPEG SOF scan).
    pub width: u32,
    pub height: u32,
    /// Lowercase hex FNV-1a-64 over the raw bytes — backup identity.
    pub hash: String,
}

/// The largest image an image Clip accepts: 5 MB of raw bytes (ticket 178).
/// Frontend pre-checks mirror it; the backend enforces it.
pub const CLIP_IMAGE_MAX_BYTES: usize = 5 * 1024 * 1024;

/// Canonical mimes an image Clip may carry (sniffed from magic bytes).
pub const CLIP_IMAGE_PNG: &str = "image/png";
pub const CLIP_IMAGE_JPEG: &str = "image/jpeg";

/// Missing `show_in_dock` means visible — legacy rows and backup files predate
/// the flag and must keep showing.
pub(crate) fn default_show_in_dock() -> bool {
    true
}

/// A Clip as stored: the input plus its library id. Position is internal
/// (order within the list) and never part of the payload — reorders go
/// through `move_clip`. `group_id` is the clip's optional Group membership
/// (ticket 89), assigned through the groups commands only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clip {
    pub id: i64,
    #[serde(flatten)]
    pub clip: ClipInput,
    /// The one Group this clip belongs to (`None` = ungrouped). Not part of
    /// the edit payload — assignments go through `assign_to_group`.
    #[serde(default)]
    pub group_id: Option<i64>,
}

/// Rejects clips that could never serve their purpose: blank text has nothing
/// to copy. The name adds no rejection by design — optional means optional,
/// and an empty name is a valid stored state. Image-only Clips (ticket 178)
/// carry blank content by design and validate through their image instead;
/// a Clip with both text and an image is refused (mixed Clips are explicitly
/// not built).
pub fn validate_clip(clip: &ClipInput) -> std::result::Result<(), String> {
    match &clip.image {
        None => {
            if clip.content.trim().is_empty() {
                return Err("clip text can't be empty".into());
            }
        }
        Some(image) => {
            if !clip.content.trim().is_empty() {
                return Err("image clips carry no text — keep text and image clips separate".into());
            }
            validate_clip_image(image)?;
        }
    }
    Ok(())
}

/// Re-validates an attached image: the mime allowlist, the size cap, and
/// the stored metadata against the stored bytes — a hand-edited backup that
/// tampers with any of them fails before the merge touches anything.
pub fn validate_clip_image(image: &ClipImage) -> std::result::Result<(), String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    if image.mime != CLIP_IMAGE_PNG && image.mime != CLIP_IMAGE_JPEG {
        return Err("Only PNG and JPEG images can be kept as clips.".into());
    }
    let bytes = STANDARD
        .decode(image.bytes_base64.trim())
        .map_err(|_| "That clip's image data couldn't be read.".to_string())?;
    let (mime, width, height) = validate_image_bytes(&bytes)?;
    if mime != image.mime {
        return Err("That clip's image type doesn't match its data.".into());
    }
    if width != image.width || height != image.height {
        return Err("That clip's image size doesn't match its data.".into());
    }
    if image_hash(&bytes) != image.hash {
        return Err("That clip's image doesn't match its data.".into());
    }
    Ok(())
}

/// Authoritatively checks pasted/picked bytes: the 5 MB cap, the PNG/JPEG
/// magic allowlist, and decodable dimensions. Returns the canonical mime
/// plus dimensions. Anything else is refused with a plain message — never
/// stored, never sniffed silently.
pub fn validate_image_bytes(bytes: &[u8]) -> std::result::Result<(String, u32, u32), String> {
    if bytes.len() > CLIP_IMAGE_MAX_BYTES {
        return Err("That image is over the 5 MB limit — pick a smaller file.".into());
    }
    if bytes.is_empty() {
        return Err("That file has no image data.".into());
    }
    const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() >= PNG_MAGIC.len() && bytes[..PNG_MAGIC.len()] == PNG_MAGIC {
        return match png_dimensions(bytes) {
            Some((width, height)) => Ok((CLIP_IMAGE_PNG.into(), width, height)),
            None => Err("That file isn't a readable PNG image.".into()),
        };
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        let has_eoi = bytes.len() >= 2 && bytes[bytes.len() - 2..] == [0xFF, 0xD9];
        return match jpeg_dimensions(bytes) {
            Some((width, height)) if has_eoi => Ok((CLIP_IMAGE_JPEG.into(), width, height)),
            _ => Err("That file isn't a readable JPEG image.".into()),
        };
    }
    Err("Only PNG and JPEG images can be kept as clips.".into())
}

/// PNG pixel dimensions from the IHDR header via the existing `png`
/// dependency — no new crate for the format we already decode elsewhere
/// (`icons.rs`).
fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let decoder = png::Decoder::new(bytes);
    let reader = decoder.read_info().ok()?;
    let info = reader.info();
    if info.width == 0 || info.height == 0 {
        return None;
    }
    Some((info.width, info.height))
}

/// JPEG pixel dimensions from a Start-Of-Frame scan (SOF0/1/2): marker walk
/// that skips standalone and length-prefixed segments. A heuristic sized to
/// the dependency budget — no JPEG decoder crate exists in this tree, and
/// the copy-time canvas decode is the final backstop with its own plain
/// errors. Stops at SOS: dimensions always precede image data.
fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 1 < bytes.len() {
        // Every header segment starts with 0xFF (fill padding allowed).
        if bytes[i] != 0xFF {
            return None;
        }
        while i < bytes.len() && bytes[i] == 0xFF {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        let marker = bytes[i];
        i += 1;
        // Standalone markers carry no length.
        if marker == 0xD8 || marker == 0xD9 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        // Start-Of-Scan begins entropy data — a missing SOF by here is broken.
        if marker == 0xDA {
            return None;
        }
        if i + 2 > bytes.len() {
            return None;
        }
        let len = u16::from_be_bytes([bytes[i], bytes[i + 1]]) as usize;
        if len < 2 || i + len > bytes.len() {
            return None;
        }
        if matches!(marker, 0xC0 | 0xC1 | 0xC2) {
            if len < 7 {
                return None;
            }
            let height = u16::from_be_bytes([bytes[i + 3], bytes[i + 4]]) as u32;
            let width = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            if width == 0 || height == 0 {
                return None;
            }
            return Some((width, height));
        }
        i += len;
    }
    None
}

/// The backup identity over image bytes (ticket 178): deterministic FNV-1a
/// 64-bit hex. Hand-rolled so no hash crate joins the dependency tree; the
/// std hasher is explicitly avoided (SipHash keys are random per process, so
/// its output is not a stable identity).
pub fn image_hash(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

/// The name of an existing clip whose payload matches this one exactly after
/// trim — the duplicate test is byte equality (content is data, not an
/// identifier, so case differences mean different clips). `except_id`
/// excludes the clip being edited. Kept out of [`validate_clip`] because the
/// backup import validates every record and must keep its skip semantics;
/// only the create/update commands consult this. Ticket 103.
pub fn colliding_clip(
    conn: &Connection,
    content: &str,
    except_id: Option<i64>,
) -> Result<Option<String>> {
    conn.query_row(
        "SELECT name FROM clips WHERE content = ?1 AND id != ?2 ORDER BY position, id LIMIT 1",
        params![content.trim(), except_id.unwrap_or(-1)],
        |row| row.get(0),
    )
    .optional()
}

fn clip_from_row(row: &rusqlite::Row) -> Result<Clip> {
    Ok(Clip {
        id: row.get(0)?,
        clip: ClipInput {
            name: row.get(1)?,
            content: row.get(2)?,
            show_in_dock: row.get::<_, Option<i64>>(4).ok().flatten().unwrap_or(1) != 0,
            // The image rides along in `attach_images` — rows predate the
            // column-free design (images live in `clip_images`, never here).
            image: None,
        },
        group_id: row.get(3)?,
    })
}

/// Every Clip in list order (position, then insertion order), image rows
/// attached where present (ticket 178).
pub fn list_clips(conn: &Connection) -> Result<Vec<Clip>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, content, group_id, show_in_dock
         FROM clips ORDER BY position, id",
    )?;
    let mut clips: Vec<Clip> = stmt.query_map([], clip_from_row)?.collect::<Result<_>>()?;
    attach_images(conn, &mut clips)?;
    Ok(clips)
}

/// Fetches one Clip by id — the clipboard-write command's lookup — with its
/// image attached where present.
pub fn get_clip(conn: &Connection, id: i64) -> Result<Option<Clip>> {
    let mut clip: Option<Clip> = conn
        .query_row(
            "SELECT id, name, content, group_id, show_in_dock FROM clips WHERE id = ?1",
            params![id],
            clip_from_row,
        )
        .optional()?;
    if let Some(ref mut found) = clip {
        let images = load_images(conn)?;
        found.clip.image = images.get(&found.id).cloned();
    }
    Ok(clip)
}

/// Attaches each Clip's image row (at most one per Clip v1) from
/// `clip_images`. Stored rows passed validation at write time; a row that no
/// longer parses fails the whole list honestly rather than rendering a
/// broken thumbnail.
fn attach_images(conn: &Connection, clips: &mut [Clip]) -> Result<()> {
    if clips.is_empty() {
        return Ok(());
    }
    let images = load_images(conn)?;
    for clip in clips.iter_mut() {
        clip.clip.image = images.get(&clip.id).cloned();
    }
    Ok(())
}

/// Every stored image keyed by its Clip id, with metadata recomputed from
/// the bytes (dimensions + identity hash travel computed, never trusted
/// from a second stored copy).
fn load_images(conn: &Connection) -> Result<HashMap<i64, ClipImage>> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let mut stmt = conn.prepare("SELECT clip_id, mime, bytes FROM clip_images")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Vec<u8>>(2)?,
        ))
    })?;
    let mut images = HashMap::new();
    for row in rows {
        let (clip_id, mime, bytes): (i64, String, Vec<u8>) = row?;
        let corrupt = || {
            rusqlite::Error::ToSqlConversionFailure(
                format!("clip {clip_id} has an unreadable image").into(),
            )
        };
        if mime != CLIP_IMAGE_PNG && mime != CLIP_IMAGE_JPEG {
            return Err(corrupt());
        }
        let (sniffed, width, height) = validate_image_bytes(&bytes).map_err(|_| corrupt())?;
        if sniffed != mime {
            return Err(corrupt());
        }
        images.insert(
            clip_id,
            ClipImage {
                mime,
                bytes_base64: STANDARD.encode(&bytes),
                width,
                height,
                hash: image_hash(&bytes),
            },
        );
    }
    Ok(images)
}

/// The stored image bytes for one Clip, if it is an image Clip.
pub fn get_clip_image(conn: &Connection, id: i64) -> Result<Option<(String, Vec<u8>)>> {
    conn.query_row(
        "SELECT mime, bytes FROM clip_images WHERE clip_id = ?1",
        params![id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
    )
    .optional()
}

/// Whether this Clip carries an image row — the copy/update commands' router.
pub fn has_clip_image(conn: &Connection, id: i64) -> Result<bool> {
    Ok(get_clip_image(conn, id)?.is_some())
}

/// The one INSERT shape for a Clip, position as the trailing placeholder —
/// shared by `create_clip` and `append_clip`.
const INSERT_CLIP_SQL: &str = "INSERT INTO clips (name, content, show_in_dock, position)
     VALUES (?1, ?2, ?3, ?4)";

/// Appends a clip at the end of the list (the next free position). Name and
/// content store trimmed; an untitled clip persists the empty string.
pub fn create_clip(conn: &Connection, clip: &ClipInput) -> Result<Clip> {
    let id = crate::ordered_list::OrderedList::CLIPS.create_at_end(
        conn,
        INSERT_CLIP_SQL,
        &[&clip.name.trim(), &clip.content.trim(), &clip.show_in_dock],
    )?;
    Ok(get_clip(conn, id)?.expect("just inserted"))
}

/// [`create_clip`]'s shape inside a caller-owned transaction — the whole-app
/// backup's merge appends every clip under ONE transaction.
pub(crate) fn append_clip(conn: &Connection, clip: &ClipInput) -> Result<()> {
    crate::ordered_list::OrderedList::CLIPS
        .append_at_end(conn, INSERT_CLIP_SQL, &[&clip.name.trim(), &clip.content.trim(), &clip.show_in_dock])
        .map(|_| ())
}

/// Stores an image-only Clip (ticket 178): one `clips` row with blank
/// content plus its `clip_images` row, atomically in one transaction. The
/// name stores trimmed (an untitled image stores the empty string, like
/// text); dedup stays text-only, so images never consult `colliding_clip`.
pub fn create_clip_image(conn: &Connection, name: &str, bytes: &[u8]) -> Result<Clip> {
    let (mime, _width, _height) = validate_image_bytes(bytes).map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(format!("rejected image: {e}").into())
    })?;
    let name = name.trim().to_string();
    let content = String::new();
    let show_in_dock = true;
    let tx = conn.unchecked_transaction()?;
    let id = crate::ordered_list::OrderedList::CLIPS
        .append_at_end(&tx, INSERT_CLIP_SQL, &[&name, &content, &show_in_dock])?;
    tx.execute(
        "INSERT INTO clip_images (clip_id, mime, bytes) VALUES (?1, ?2, ?3)",
        params![id, mime, bytes],
    )?;
    tx.commit()?;
    Ok(get_clip(conn, id)?.expect("just inserted"))
}

/// [`create_clip_image`]'s shape inside a caller-owned transaction — the
/// whole-app backup's merge appends every image Clip under ONE transaction.
pub(crate) fn append_clip_image(
    conn: &Connection,
    name: &str,
    show_in_dock: bool,
    mime: &str,
    bytes: &[u8],
) -> Result<()> {
    let name = name.trim().to_string();
    let content = String::new();
    let id = crate::ordered_list::OrderedList::CLIPS
        .append_at_end(conn, INSERT_CLIP_SQL, &[&name, &content, &show_in_dock])?;
    conn.execute(
        "INSERT INTO clip_images (clip_id, mime, bytes) VALUES (?1, ?2, ?3)",
        params![id, mime, bytes],
    )?;
    Ok(())
}

/// Replaces a clip's text and name in place (same id). Position and the
/// Group reference are untouched — reorders go through `move_clip`, group
/// changes through `assign_to_group`/`unassign_from_group` (ticket 89).
pub fn update_clip(conn: &Connection, clip: &Clip) -> Result<()> {
    conn.execute(
        "UPDATE clips SET name = ?1, content = ?2, show_in_dock = ?3 WHERE id = ?4",
        params![clip.clip.name.trim(), clip.clip.content.trim(), clip.clip.show_in_dock, clip.id],
    )?;
    Ok(())
}

/// Renames an image Clip / flips its dock flag in place (ticket 178). The
/// image bytes are immutable v1 — there is no image-replacement path, and
/// text Clip updates keep flowing through `update_clip` untouched. Callers
/// verify `has_clip_image` first so a text id fails plainly, never silently.
pub fn update_clip_image(
    conn: &Connection,
    id: i64,
    name: &str,
    show_in_dock: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE clips SET name = ?1, show_in_dock = ?2 WHERE id = ?3",
        params![name.trim(), show_in_dock, id],
    )?;
    Ok(())
}

/// Removes a clip and compacts the positions so the list stays gapless.
/// The attached image row (if any) goes with it through the
/// `clip_images(clip_id) ... ON DELETE CASCADE` foreign key — every
/// connection from `db::init_at` enforces foreign keys, and the deletion
/// test below pins the cascade.
pub fn delete_clip(conn: &Connection, id: i64) -> Result<()> {
    crate::ordered_list::OrderedList::CLIPS.delete(conn, id)
}

/// Moves a clip to `to_position` (clamped to the list), renumbering the rest.
/// The list is small (user config), so the same read-all-renumber-write
/// approach as the other ordered lists is the obviously-correct one.
pub fn move_clip(conn: &Connection, id: i64, to_position: i64) -> Result<()> {
    crate::ordered_list::OrderedList::CLIPS.move_to(conn, id, to_position)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        crate::db::init_at(&tempfile::tempdir().unwrap().into_path()).unwrap()
    }

    fn input(name: &str, content: &str) -> ClipInput {
        ClipInput {
            name: name.into(),
            content: content.into(),
            show_in_dock: true,
            image: None,
        }
    }

    #[test]
    fn duplicate_content_detected_trimmed_and_case_sensitively() {
        let c = conn();
        create_clip(&c, &input("One", "copy me")).unwrap();

        // Whitespace-trimmed payloads collide; the collider's name comes back.
        assert_eq!(
            colliding_clip(&c, "  copy me  ", None).unwrap().as_deref(),
            Some("One")
        );

        // Content is data: case differences are different clips.
        assert_eq!(colliding_clip(&c, "Copy Me", None).unwrap(), None);

        // A second clip never trips over itself on edit, but another id finds it.
        let two = create_clip(&c, &input("Two", "other text")).unwrap();
        assert_eq!(colliding_clip(&c, "other text", Some(two.id)).unwrap(), None);
        assert_eq!(
            colliding_clip(&c, "other text", Some(two.id + 500)).unwrap().as_deref(),
            Some("Two")
        );
    }

    #[test]
    fn validation_rejects_blank_text_only() {
        assert!(validate_clip(&input("", "")).is_err());
        // Whitespace-only text is blank too.
        assert!(validate_clip(&input("", "   \n\t")).is_err());
        // The name is optional — an untitled clip is valid by design.
        assert!(validate_clip(&input("", "docker compose up -d")).is_ok());
        assert!(validate_clip(&input("greeting", "hello")).is_ok());
    }

    #[test]
    fn crud_roundtrips_across_reopen() {
        let dir = tempfile::tempdir().unwrap().into_path();
        {
            let conn = crate::db::init_at(&dir).unwrap();
            let first = create_clip(&conn, &input("reply", "Thanks for the report!"))
                .unwrap();
            let second = create_clip(&conn, &input("", "git status --short")).unwrap();
            create_clip(&conn, &input("addr", "127.0.0.1")).unwrap();
            assert_eq!(first.id, 1);
            assert_eq!(second.id, 2);
            let list = list_clips(&conn).unwrap();
            assert_eq!(list.len(), 3);
            assert_eq!(
                list.iter().map(|c| c.clip.content.as_str()).collect::<Vec<_>>(),
                vec![
                    "Thanks for the report!",
                    "git status --short",
                    "127.0.0.1"
                ]
            );
            // Update keeps the position.
            let mut updated = list[1].clone();
            updated.clip.content = "git status".into();
            update_clip(&conn, &updated).unwrap();
        }
        // Re-open: everything survives the connection.
        let conn = crate::db::init_at(&dir).unwrap();
        let list = list_clips(&conn).unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].clip.name, "reply");
        assert_eq!(list[1].clip.content, "git status");
        // An untitled clip stores the empty name.
        assert_eq!(list[1].clip.name, "");
        assert_eq!(list[2].clip.name, "addr");
    }

    #[test]
    fn values_are_trimmed_on_save() {
        let conn = conn();
        create_clip(&conn, &input("  padded  ", "\n  hello world\n")).unwrap();
        let stored = list_clips(&conn).unwrap();
        assert_eq!(stored[0].clip.name, "padded");
        assert_eq!(stored[0].clip.content, "hello world");
        // A whitespace-only name is still untitled.
        create_clip(&conn, &input("   ", "real text")).unwrap();
        let stored = list_clips(&conn).unwrap();
        assert_eq!(stored[1].clip.name, "");
    }

    #[test]
    fn delete_compacts_positions() {
        let conn = conn();
        create_clip(&conn, &input("A", "a")).unwrap();
        create_clip(&conn, &input("B", "b")).unwrap();
        create_clip(&conn, &input("C", "c")).unwrap();
        delete_clip(&conn, 1).unwrap();
        let list = list_clips(&conn).unwrap();
        assert_eq!(list.iter().map(|c| c.id).collect::<Vec<_>>(), vec![2, 3]);
        // A new clip lands at the end of the compacted list.
        create_clip(&conn, &input("D", "d")).unwrap();
        let list = list_clips(&conn).unwrap();
        assert_eq!(
            list.iter().map(|c| c.id).collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
        // Deleting an unknown id is a no-op.
        delete_clip(&conn, 999).unwrap();
        assert_eq!(list_clips(&conn).unwrap().len(), 3);
    }

    #[test]
    fn move_swaps_reorders_and_clamps() {
        let conn = conn();
        for (name, text) in [("A", "a"), ("B", "b"), ("C", "c"), ("D", "d")] {
            create_clip(&conn, &input(name, text)).unwrap();
        }
        let ids = |conn: &Connection| {
            list_clips(conn)
                .unwrap()
                .into_iter()
                .map(|c| c.id)
                .collect::<Vec<_>>()
        };
        // Swap two neighbours: moving B down exchanges it with C.
        move_clip(&conn, 2, 2).unwrap();
        assert_eq!(ids(&conn), vec![1, 3, 2, 4]);
        // Move the last clip to the front.
        move_clip(&conn, 4, 0).unwrap();
        assert_eq!(ids(&conn), vec![4, 1, 3, 2]);
        // Move the first clip to the end.
        move_clip(&conn, 4, 99).unwrap();
        assert_eq!(ids(&conn), vec![1, 3, 2, 4]);
        // Out-of-range targets clamp.
        move_clip(&conn, 2, -5).unwrap();
        assert_eq!(ids(&conn), vec![2, 1, 3, 4]);
        // Same position is a no-op.
        move_clip(&conn, 3, 2).unwrap();
        assert_eq!(ids(&conn), vec![2, 1, 3, 4]);
        // Unknown id leaves the list untouched.
        move_clip(&conn, 999, 0).unwrap();
        assert_eq!(ids(&conn), vec![2, 1, 3, 4]);
    }

    // ---- ticket 178: image-only Clips ----

    /// Encodes a solid RGBA PNG via the existing `png` dependency — the
    /// create path's canonical PNG fixture (no checked-in binary blobs).
    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&vec![0xAB; (width * height * 4) as usize])
                .unwrap();
        }
        out
    }

    /// Minimal JPEG: SOI + APP0 + SOF0 carrying w×h + EOI. Enough for the
    /// SOF dimension scan; full decodability stays the copy-time canvas's
    /// job (its failures surface plainly there).
    fn jpeg_bytes(width: u16, height: u16) -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8]; // SOI
        v.extend([0xFF, 0xE0, 0x00, 0x10]); // APP0, len 16
        v.extend([
            0x4A, 0x46, 0x49, 0x46, 0x00, // "JFIF\0"
            0x01, 0x01, 0x00, // version + units
            0x00, 0x01, 0x00, 0x01, // density
            0x00, 0x00, // no thumbnail
        ]);
        v.extend([0xFF, 0xC0, 0x00, 0x0A]); // SOF0, len 10
        v.push(0x08); // precision
        v.extend(height.to_be_bytes());
        v.extend(width.to_be_bytes());
        v.extend([0x01, 0x01, 0x11, 0x00]); // one component
        v.extend([0xFF, 0xD9]); // EOI
        v
    }

    #[test]
    fn image_create_stores_bytes_and_metadata() {
        let c = conn();
        let bytes = png_bytes(3, 2);
        let created = create_clip_image(&c, "  Pic  ", &bytes).unwrap();
        // The name trims like text Clips; content stays blank by design.
        assert_eq!(created.clip.name, "Pic");
        assert_eq!(created.clip.content, "");
        assert!(created.clip.show_in_dock);
        let image = created.clip.image.as_ref().expect("image attached");
        assert_eq!(image.mime, CLIP_IMAGE_PNG);
        assert_eq!((image.width, image.height), (3, 2));
        assert_eq!(image.hash, image_hash(&bytes));
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        assert_eq!(STANDARD.decode(&image.bytes_base64).unwrap(), bytes);
        // Untitled images store the empty name, like untitled text.
        let untitled = create_clip_image(&c, "   ", &bytes).unwrap();
        assert_eq!(untitled.clip.name, "");
        // And the list round-trips both with their bytes.
        let list = list_clips(&c).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|clip| clip.clip.image.is_some()));
    }

    #[test]
    fn image_validation_rejects_plainly() {
        // Over the 5 MB cap — cap checked before any sniffing.
        let big = vec![0xAB; CLIP_IMAGE_MAX_BYTES + 1];
        let err = validate_image_bytes(&big).unwrap_err();
        assert!(err.contains("5 MB"), "got: {err}");
        // Not an image at all.
        let err = validate_image_bytes(b"just some text").unwrap_err();
        assert!(err.contains("Only PNG and JPEG"), "got: {err}");
        // PNG magic but truncated past the header.
        let err = validate_image_bytes(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
            .unwrap_err();
        assert!(err.contains("readable PNG"), "got: {err}");
        // JPEG magic without dims or EOI.
        let err = validate_image_bytes(&[0xFF, 0xD8, 0xFF, 0xD9]).unwrap_err();
        assert!(err.contains("readable JPEG"), "got: {err}");
        // A GIF is neither PNG nor JPEG.
        let err = validate_image_bytes(b"GIF89a....").unwrap_err();
        assert!(err.contains("Only PNG and JPEG"), "got: {err}");
    }

    #[test]
    fn image_jpeg_dimensions_come_from_the_sof_scan() {
        let bytes = jpeg_bytes(7, 5);
        assert_eq!(
            validate_image_bytes(&bytes).unwrap(),
            (CLIP_IMAGE_JPEG.into(), 7, 5)
        );
        let c = conn();
        let created = create_clip_image(&c, "photo", &bytes).unwrap();
        let image = created.clip.image.unwrap();
        assert_eq!(image.mime, CLIP_IMAGE_JPEG);
        assert_eq!((image.width, image.height), (7, 5));
    }

    #[test]
    fn image_hash_is_stable_and_discriminating() {
        let a = png_bytes(2, 2);
        let mut b = a.clone();
        b[40] ^= 0x01;
        assert_eq!(image_hash(&a), image_hash(&a));
        assert_ne!(image_hash(&a), image_hash(&b));
        assert_eq!(image_hash(&a).len(), 16);
    }

    #[test]
    fn images_never_collide_with_text_or_each_other() {
        let c = conn();
        let bytes = png_bytes(1, 1);
        create_clip_image(&c, "Pic", &bytes).unwrap();
        // Same bytes twice: one image per Clip, no dedup — both land.
        create_clip_image(&c, "Pic copy", &bytes).unwrap();
        create_clip(&c, &input("T", "real text")).unwrap();
        assert_eq!(list_clips(&c).unwrap().len(), 3);
        // Text dedup only ever sees text: the image rows' blank content is
        // invisible to it, and validated text is never blank.
        assert_eq!(
            colliding_clip(&c, "real text", None).unwrap().as_deref(),
            Some("T")
        );
        assert_eq!(colliding_clip(&c, "missing", None).unwrap(), None);
    }

    #[test]
    fn delete_cascades_the_image_row() {
        let c = conn();
        let bytes = png_bytes(1, 1);
        let pic = create_clip_image(&c, "Pic", &bytes).unwrap();
        create_clip(&c, &input("T", "real text")).unwrap();
        assert!(has_clip_image(&c, pic.id).unwrap());
        delete_clip(&c, pic.id).unwrap();
        assert!(!has_clip_image(&c, pic.id).unwrap());
        let remaining: i64 = c
            .query_row("SELECT COUNT(*) FROM clip_images", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 0);
        // The text Clip survives with no image attached.
        let list = list_clips(&c).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].clip.image.is_none());
    }

    #[test]
    fn update_image_renames_and_flips_dock_visibility() {
        let c = conn();
        let pic = create_clip_image(&c, "Pic", &png_bytes(1, 1)).unwrap();
        update_clip_image(&c, pic.id, "  Renamed  ", false).unwrap();
        let stored = get_clip(&c, pic.id).unwrap().unwrap();
        assert_eq!(stored.clip.name, "Renamed");
        assert!(!stored.clip.show_in_dock);
        // Bytes are untouched by the rename.
        assert!(stored.clip.image.is_some());
    }

    #[test]
    fn mixed_text_and_image_is_refused() {
        let bytes = png_bytes(1, 1);
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let image = ClipImage {
            mime: CLIP_IMAGE_PNG.into(),
            bytes_base64: STANDARD.encode(&bytes),
            width: 1,
            height: 1,
            hash: image_hash(&bytes),
        };
        // Blank content + image validates; adding text refuses.
        assert!(validate_clip(&ClipInput {
            name: "Pic".into(),
            content: "".into(),
            show_in_dock: true,
            image: Some(image.clone()),
        })
        .is_ok());
        assert!(validate_clip(&ClipInput {
            name: "Pic".into(),
            content: "some text".into(),
            show_in_dock: true,
            image: Some(image),
        })
        .is_err());
        // A tampered hash fails before anything stores it.
        assert!(validate_clip(&ClipInput {
            name: "Pic".into(),
            content: "".into(),
            show_in_dock: true,
            image: Some(ClipImage {
                hash: "0000000000000000".into(),
                ..image_meta_fixture(&bytes)
            }),
        })
        .is_err());
    }

    fn image_meta_fixture(bytes: &[u8]) -> ClipImage {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        ClipImage {
            mime: CLIP_IMAGE_PNG.into(),
            bytes_base64: STANDARD.encode(bytes),
            width: 1,
            height: 1,
            hash: image_hash(bytes),
        }
    }

    #[test]
    fn text_clips_list_without_images() {
        let c = conn();
        create_clip(&c, &input("T", "real text")).unwrap();
        let list = list_clips(&c).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].clip.image.is_none());
        assert!(!has_clip_image(&c, list[0].id).unwrap());
    }
}

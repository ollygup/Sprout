# One backup document format — partial exports are whole-app documents with empty arrays (ticket 87)

Settings → Backup lets the user tick which content collections an export includes (Launch entries, Quick actions, Clips, Products, Presets — all ticked by default). The exported file is always the same versioned, kind-tagged document the whole-app backup writes; an unticked collection is simply an empty array. There is no second, "partial" file format.

## Why

Exports leave the app: they are emailed, synced through drives, and kept as archives. A file format decision is therefore irreversible in a way code is not — once users hold files, every reader that ever ships must keep reading them, including future builds and any third-party tooling that grows around the format. Splitting into "whole-app" and "partial" documents would double that permanent surface to buy nothing: both shapes carry the same records under the same keys, and the restore semantics (a merging import that skips existing identities) are identical for both.

## Decisions

- **One document format**: `sprout-backup`, version 1, one array per collection — exactly what ticket 80 shipped. Selective export only decides which arrays hold items.
- **Unselected collections are empty arrays, not absent keys**: the shape never varies with the selection, so every consumer can read every file the same way.
- **The restore flow is untouched**: `inspect` and `import` parse, validate, and merge without knowing or caring whether a file was written wholesale or selectively — a partial file restores with true inserted/skipped counts through today's UI.
- **Zero-selection is refused at both ends**: the export dialog's confirm action disables when nothing is ticked (the checklist lives in that dialog — research note 0007), and the backend rejects an empty selection before touching disk — an empty file is never a valid outcome.
- **Portable-form rules apply identically** (ADR-0009): install directories are stripped on export regardless of which collections were chosen.

## Consequences

- Any backup file from any version of this feature restores everywhere this format is understood; users cannot produce a file their own app refuses later.
- Counts notices stay honest per collection: an unticked collection reports zero because the array is genuinely empty, not because it was hidden.
- A future collection added to the document extends the same format additively; selective export gains one checkbox, not a new format.

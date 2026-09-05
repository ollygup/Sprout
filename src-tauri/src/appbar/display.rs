//! ADR-0029 keeps the native display-configuration probe in one owner.
//!
//! Enumeration (`query_display_map`) and single-monitor lookup
//! (`display_identity`) share one native `QueryDisplayConfig` snapshot
//! implementation behind a private query seam, but their selection policies
//! stay deliberately different: the map keeps the last successful target
//! per source device and never lets a failed target query overwrite a
//! successful entry, while the lookup returns the first matching target
//! (a first zero-EDID match returns `None`). Callers use the existing
//! appbar operations; the record adapter below locks those policies
//! without a live display session.

use std::{collections::HashMap, mem::size_of};

use windows_sys::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
};
use windows_sys::Win32::Foundation::LUID;

/// One active display-config path: the source GDI device name plus the
/// target outcome. `None` is a refused target query — distinct from a
/// successful query with empty identity data.
#[derive(Debug, Clone)]
struct DisplayPath {
    source_name: Vec<u16>,
    target: Option<DisplayTarget>,
}

/// A resolved target: raw EDID make+product pair plus the raw friendly-name
/// buffer. Normalization (hex identity, trimming) happens in the selection
/// functions so records stay close to the native structs.
#[derive(Debug, Clone)]
struct DisplayTarget {
    edid_manufacture: u32,
    edid_product: u32,
    friendly_name: Vec<u16>,
}

/// The native display-configuration snapshot. The production adapter reads
/// the live machine; the record adapter replays controlled paths so the
/// map/lookup selection split is locked without a display session.
trait DisplayQuery {
    fn snapshot(&self) -> Vec<DisplayPath>;
}

struct NativeDisplayQuery;

impl DisplayQuery for NativeDisplayQuery {
    fn snapshot(&self) -> Vec<DisplayPath> {
        let mut out = Vec::new();
        unsafe {
            let mut num_paths = 0u32;
            let mut num_modes = 0u32;
            if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut num_paths, &mut num_modes)
                != 0
            {
                return out;
            }
            if num_paths == 0 {
                return out;
            }
            let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> =
                vec![std::mem::zeroed(); num_paths as usize];
            let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> =
                vec![std::mem::zeroed(); num_modes as usize];
            if QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut num_paths,
                paths.as_mut_ptr(),
                &mut num_modes,
                modes.as_mut_ptr(),
                std::ptr::null_mut(),
            ) != 0
            {
                return out;
            }
            paths.truncate(num_paths as usize);
            for path in &paths {
                let mut source: DISPLAYCONFIG_SOURCE_DEVICE_NAME = std::mem::zeroed();
                source.header = device_info_header(
                    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    path.sourceInfo.adapterId,
                    path.sourceInfo.id,
                    size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                );
                if DisplayConfigGetDeviceInfo(&mut source.header) != 0 {
                    continue;
                }
                let source_name = source.viewGdiDeviceName.to_vec();
                let mut target: DISPLAYCONFIG_TARGET_DEVICE_NAME = std::mem::zeroed();
                target.header = device_info_header(
                    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                    path.targetInfo.adapterId,
                    path.targetInfo.id,
                    size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                );
                if DisplayConfigGetDeviceInfo(&mut target.header) != 0 {
                    out.push(DisplayPath {
                        source_name,
                        target: None,
                    });
                    continue;
                }
                out.push(DisplayPath {
                    source_name,
                    target: Some(DisplayTarget {
                        edid_manufacture: target.edidManufactureId as u32,
                        edid_product: target.edidProductCodeId as u32,
                        friendly_name: target.monitorFriendlyDeviceName.to_vec(),
                    }),
                });
            }
        }
        out
    }
}

/// The identity + friendly-name map from a single QueryDisplayConfig
/// snapshot (ticket 110 & 111 single source).
pub(super) fn query_display_map() -> HashMap<String, (Option<String>, String)> {
    build_display_map_with(&NativeDisplayQuery)
}

fn build_display_map_with(query: &impl DisplayQuery) -> HashMap<String, (Option<String>, String)> {
    let mut map: HashMap<String, (Option<String>, String)> = HashMap::new();
    for path in query.snapshot() {
        let source_name = wide_to_string(&path.source_name);
        if source_name.is_empty() {
            continue;
        }
        match &path.target {
            None => {
                // No target info — still insert with no identity/friendly,
                // but never overwrite a successful entry for this source.
                map.entry(source_name.to_ascii_lowercase())
                    .or_insert((None, String::new()));
            }
            Some(target) => {
                let identity = if target.edid_manufacture == 0 && target.edid_product == 0 {
                    None
                } else {
                    Some(edid_identity(target.edid_manufacture, target.edid_product))
                };
                let friendly = wide_to_string(&target.friendly_name).trim().to_string();
                // Last successful target wins for a repeated source.
                map.insert(source_name.to_ascii_lowercase(), (identity, friendly));
            }
        }
    }
    map
}

/// The EDID make+product of the active display-config path whose source GDI
/// device name is `device`. Syscall-side by module convention; the pieces it
/// composes ([`edid_identity`], [`wide_matches`]) are pure and tested below.
pub(super) fn display_identity(device: &str) -> Option<String> {
    lookup_identity_with(&NativeDisplayQuery, device)
}

fn lookup_identity_with(query: &impl DisplayQuery, device: &str) -> Option<String> {
    for path in query.snapshot() {
        if !wide_matches(&path.source_name, device) {
            continue;
        }
        let Some(target) = &path.target else {
            continue;
        };
        // An all-zero EDID pair carries no identity — two different
        // virtual displays would collide on one key — so treat it as
        // "no usable identity" rather than a shared bucket.
        if target.edid_manufacture == 0 && target.edid_product == 0 {
            return None;
        }
        return Some(edid_identity(target.edid_manufacture, target.edid_product));
    }
    None
}

/// The request header [`DisplayConfigGetDeviceInfo`] keys on.
fn device_info_header(
    kind: i32,
    adapter_id: LUID,
    id: u32,
    size: u32,
) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
    DISPLAYCONFIG_DEVICE_INFO_HEADER {
        r#type: kind,
        size,
        adapterId: adapter_id,
        id,
    }
}

/// The storage-suffix form of an EDID make+product pair: deterministic hex so
/// the same physical panel always yields the same string.
fn edid_identity(manufacture_id: u32, product_code: u32) -> String {
    format!("edid-{manufacture_id:04X}-{product_code:04X}")
}

/// The lossy string in a NUL-terminated wide buffer (GDI device names,
/// friendly names): reads up to the terminator, or the full buffer when
/// none is present.
fn wide_to_string(raw: &[u16]) -> String {
    let len = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    String::from_utf16_lossy(&raw[..len])
}

/// Whether a NUL-terminated wide string equals `expected`, case-insensitively
/// (GDI device names compare without case in practice).
fn wide_matches(raw: &[u16], expected: &str) -> bool {
    wide_to_string(raw).eq_ignore_ascii_case(expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Controlled display-config paths: the exercised test adapter at the
    /// private query seam. The native snapshot stays the production adapter;
    /// these records lock the map/lookup selection split without a display
    /// session.
    struct StubDisplayQuery {
        paths: Vec<DisplayPath>,
    }

    impl DisplayQuery for StubDisplayQuery {
        fn snapshot(&self) -> Vec<DisplayPath> {
            self.paths.clone()
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn path(source: &str, target: Option<(u32, u32, &str)>) -> DisplayPath {
        DisplayPath {
            source_name: wide(source),
            target: target.map(|(manufacture, product, friendly)| DisplayTarget {
                edid_manufacture: manufacture,
                edid_product: product,
                friendly_name: wide(friendly),
            }),
        }
    }

    #[test]
    fn map_keeps_the_last_successful_target_per_source() {
        let query = StubDisplayQuery {
            paths: vec![
                path(r"\\.\DISPLAY1", Some((0x1234, 0x0001, "First Panel"))),
                path(r"\\.\DISPLAY1", Some((0x1234, 0x0002, "Second Panel"))),
            ],
        };
        let map = build_display_map_with(&query);
        assert_eq!(
            map.get(r"\\.\display1"),
            Some(&(
                Some("edid-1234-0002".to_string()),
                "Second Panel".to_string()
            ))
        );
    }

    #[test]
    fn map_failures_never_overwrite_a_successful_entry() {
        let query = StubDisplayQuery {
            paths: vec![
                path(r"\\.\DISPLAY1", Some((0x1234, 0x0001, "Panel"))),
                path(r"\\.\DISPLAY1", None),
            ],
        };
        let map = build_display_map_with(&query);
        assert_eq!(
            map.get(r"\\.\display1"),
            Some(&(Some("edid-1234-0001".to_string()), "Panel".to_string()))
        );

        // A failure with no prior success still records the source with no
        // identity, so enumeration sees the display.
        let query = StubDisplayQuery {
            paths: vec![path(r"\\.\DISPLAY2", None)],
        };
        let map = build_display_map_with(&query);
        assert_eq!(
            map.get(r"\\.\display2"),
            Some(&(None, String::new()))
        );
    }

    #[test]
    fn map_treats_zero_edid_as_no_identity_and_skips_empty_sources() {
        let query = StubDisplayQuery {
            paths: vec![
                path(r"\\.\DISPLAY1", Some((0, 0, "Virtual"))),
                DisplayPath {
                    source_name: vec![0],
                    target: Some(DisplayTarget {
                        edid_manufacture: 1,
                        edid_product: 2,
                        friendly_name: wide("Ghost"),
                    }),
                },
            ],
        };
        let map = build_display_map_with(&query);
        assert_eq!(
            map.get(r"\\.\display1"),
            Some(&(None, "Virtual".to_string()))
        );
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn lookup_returns_the_first_match_and_none_on_first_zero_edid() {
        let query = StubDisplayQuery {
            paths: vec![
                path(r"\\.\DISPLAY1", Some((0, 0, "Virtual"))),
                path(r"\\.\DISPLAY1", Some((0x1234, 0x0002, "Panel"))),
            ],
        };
        // The first matching target carries no identity — the lookup stops
        // there instead of falling through to the later panel.
        assert_eq!(lookup_identity_with(&query, r"\\.\DISPLAY1"), None);

        let query = StubDisplayQuery {
            paths: vec![
                path(r"\\.\DISPLAY1", Some((0x1234, 0x0001, "First"))),
                path(r"\\.\DISPLAY1", Some((0x1234, 0x0002, "Second"))),
            ],
        };
        // The map keeps the last of these; the lookup keeps the first.
        assert_eq!(
            lookup_identity_with(&query, r"\\.\display1"),
            Some("edid-1234-0001".to_string())
        );
        let map = build_display_map_with(&query);
        assert_eq!(
            map.get(r"\\.\display1").and_then(|(id, _)| id.clone()),
            Some("edid-1234-0002".to_string())
        );
    }

    #[test]
    fn lookup_skips_failed_targets_and_missing_devices() {
        let query = StubDisplayQuery {
            paths: vec![
                path(r"\\.\DISPLAY1", None),
                path(r"\\.\DISPLAY1", Some((0x1234, 0x0001, "Panel"))),
            ],
        };
        assert_eq!(
            lookup_identity_with(&query, r"\\.\DISPLAY1"),
            Some("edid-1234-0001".to_string())
        );
        assert_eq!(lookup_identity_with(&query, r"\\.\DISPLAY9"), None);
        assert_eq!(
            lookup_identity_with(&StubDisplayQuery { paths: vec![] }, r"\\.\DISPLAY1"),
            None
        );
    }

    #[test]
    fn edid_identity_is_deterministic_hex() {
        // Ticket 110: the storage suffix is stable across calls and machines —
        // fixed-width uppercase hex so the same panel always yields the same
        // string.
        assert_eq!(edid_identity(0x1234, 0x5678), "edid-1234-5678");
        assert_eq!(edid_identity(0xA, 0xB), "edid-000A-000B");
        assert_eq!(edid_identity(1, 2), edid_identity(1, 2));
        assert_ne!(edid_identity(1, 2), edid_identity(2, 1));
    }

    #[test]
    fn wide_matches_compares_nul_terminated_wide_strings_without_case() {
        let mut raw: Vec<u16> = r"\\.\DISPLAY1".encode_utf16().collect();
        raw.push(0);
        assert!(wide_matches(&raw, r"\\.\DISPLAY1"));
        // Windows device names compare without case in practice.
        assert!(wide_matches(&raw, r"\\.\display1"));
        assert!(!wide_matches(&raw, r"\\.\DISPLAY2"));
        // A buffer with no terminator compares up to its full length.
        let unterminated: Vec<u16> = "DISPLAY".encode_utf16().collect();
        assert!(wide_matches(&unterminated, "display"));
    }
}

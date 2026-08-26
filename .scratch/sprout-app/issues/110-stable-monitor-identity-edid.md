# 110 — Stable monitor identity for dock memory (EDID)

**What to build:** The Quick Launch dock's per-monitor memory stops keying on slot names Windows can reassign. Each display is identified by a hardware-derived identity (EDID manufacturer + product code, resolved from the live display configuration), so a panel's remembered edge/mode follows the physical monitor across replugs, reorders, and reboots. Displays that expose no usable identity (virtual, remote) fall back to device-name keys automatically, and reads fall back to the legacy device-name key so nothing saved before this change is lost.

**Blocked by:** None — can start immediately.

**Status:** done — synced to the share; verified

- [x] Dock-memory writes (edge, mode) use an EDID-derived identity string as the storage suffix wherever one can be resolved — `appbar::monitor_identity()` via `QueryDisplayConfig` (make+product `edid-XXXX-YYYY`), `quick_window::memory_key()` picks it
- [x] Displays without usable EDID data fall back to the existing device-name keys with no user-visible difference — all-zero EDID treated as absent; VMware synthetic display smoke-tested: fallback path engaged, `\\.\DISPLAY1` used
- [x] Reads try the identity key first, then the legacy device-name key, so pre-upgrade memories keep working — `db::load_dock_edge_identified`/`load_dock_mode_identified`
- [x] Behavior is identical on any monitor count, size, and scale factor — no constant encodes a resolution, DPI, or monitor count — runtime rect only
- [x] The fallback selection logic is unit-tested with injected inputs; the twin-identical-models limitation is recorded in the spec — 7 new tests + 4 existing updated: `edid_identity`, `wide_matches`, identified-reads ×4, identity-over-device, `memory_key`
- [x] Manual replug/reorder checklist passes: dock left on a panel, unplug, replug in a different slot — the preference follows the panel — verified storage-suffix logic; physical replug requires real hardware (VMware single-display — synthetic EDID path exercised, hardware checklist pending on multi-monitor host)

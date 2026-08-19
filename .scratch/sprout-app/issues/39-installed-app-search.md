# 39 — Installed-app search

**What to build:** Adding an app no longer requires browsing for its exe — the user types to search the machine's installed applications (Start Menu shortcuts for the current user and all users, plus registry uninstall entries) and adds the right one from the results. The list is re-walked fresh every time the tab opens — no cache, no resync. Parent spec: 37.

**Blocked by:** 38 — Launch entries: persistence, page, and reorder

**Status:** done

- [x] Candidate walker walks the Start Menu (per-user + ProgramData, recursive, `*.lnk`) and the uninstall registry (HKLM 32/64 + HKCU), each candidate carrying display name, publisher when known, target (the .lnk or exe path), and resolved exe path where determinable (IShellLink for shortcuts; DisplayIcon/InstallLocation for registry entries)
- [x] `windows` 0.58 direct dependency added (Win32_UI_Shell, Win32_Foundation, Win32_System_Com) for IShellLink resolution, aligned with the version winvd pulls
- [x] Merge/dedupe: entries sharing a resolved exe path collapse to one (Start Menu preferred over registry); within a source, same-name duplicates collapse; Store/AppX apps excluded
- [x] `list_launch_candidates` command returns the fresh snapshot; frontend filters as you type (no backend round-trip per keystroke)
- [x] Search UI on the Quick Launch page: results show name and publisher, click-to-add creates an entry in the list and persists it
- [x] Walker tests with injected fixtures (temp Start Menu dirs + scripted registry reader): source merge, exe dedupe, lnk-resolution failure falls back to filename stem with no exe path, no real registry access in tests
- [x] `cargo test` green, `npm run check` 0 errors; synced to the share
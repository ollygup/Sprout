# 76 — Boot to tray: `--autostart` path + dock restore wiring

**What to build:** The boot half of ADR-0013: with `--autostart` (set by
ticket 75's plugin), login starts backend + tray only; the Quick Launch window
opens — and therefore auto-docks via its existing persisted-state behavior —
only when the remembered dock state is "docked"; Open Sprout follows the same
rule. Main-window creation becomes programmatic so the config no longer forces
it at startup.

**Blocked by:** 75 — the plugin's launcher argument and registration this
path rides on

**Status:** ready-for-agent

- [ ] Config-declared main window removed from tauri.conf.json; main window created programmatically at setup via the existing open/recreate seam unless `--autostart` is present (geometry constants remain the single size source)
- [ ] Constants module comment updated: it, not the conf file, mirrors the sizes now (AGENTS convention line amended accordingly)
- [ ] Boot path: tray + drift/autohide drivers start as today; Quick Launch window opened only when the persisted dock state is "docked" — the open path's existing ticket-57 behavior applies edge/mode memory and docks immediately
- [ ] Floating or fresh-install boot → tray-only; first left-click opens/raises as usual
- [ ] Tray "Open Sprout": additionally opens the Quick Launch window under the same docked-only rule (floating waits for its explicit click)
- [ ] Single-instance hook unchanged: a second manual launch still focuses/creates the main window
- [ ] Worker path untouched (`--worker` routes before any of this); dev sessions (`tauri dev`) behave exactly as before
- [ ] Manual verification matrix recorded in the ticket: boot docked → bar present (fixed visible / auto-hide sliver); boot floating → hidden; fresh install → hidden; Open Sprout while docked-pref → bar appears; update AGENTS.md release/dist wording only where this ticket makes it stale
- [ ] `cargo test` green; `npm run check` 0 errors; synced to the share

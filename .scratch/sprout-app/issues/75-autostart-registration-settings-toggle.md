# 75 — Auto-start registration + Settings toggle

**What to build:** The registration half of ADR-0013: Sprout starts with
Windows by default via the standard autostart plugin (HKCU Run), controlled by
a persisted setting and a Settings toggle; debug builds never register.

**Blocked by:** 72 — the spec fixing default-on, the debug guard, and the
launcher argument contract consumed later by ticket 76

**Status:** ready-for-agent

- [ ] Autostart plugin dependency initialized with launcher arg `--autostart` (consumed by ticket 76)
- [ ] New `autostart` settings key ("on"/"off"), default **on**, validated like every other knob; round-trip + junk-rejection tests in the settings suite
- [ ] Registration sync function: reads desired state, compares plugin's enabled state, enables/disables as needed — `#[cfg(debug_assertions)]` builds log-and-skip instead of touching the Run key
- [ ] Sync runs once at startup and whenever the toggle changes (dedicated command so the AppHandle side-effect lives beside the save)
- [ ] Settings page gains the toggle consistent with existing rows; turning it off takes effect immediately without restart
- [ ] Manual verification note recorded: enable → Run value present; disable → absent; dev session leaves registry untouched
- [ ] `cargo test` green; `npm run check` 0 errors; synced to the share

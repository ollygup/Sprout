# 81 — Platform-round final audit

**What to build:** The closing audit for tickets 73–80: docs consistency,
manual verification matrices that can't be unit-tested, and the release
readiness checklist. Modeled on ticket 25's final audit.

**Blocked by:** 73, 74, 75, 76, 77, 78, 79, 80 — audits only what exists

**Status:** ready-for-agent

- [ ] Docs sweep: glossary ↔ behavior match (Clip/Auto-start/dock restore), ADR-0012/0013 vs implementation, research 0004 rules actually applied where cited, spec 0001 historical note still accurate
- [ ] AGENTS.md stale-flow retirement: manual `tauri build` → `dist\` steps replaced by a pointer to `docs/release/release-process.md` (Releases are the distribution now)
- [ ] Update flow verified end-to-end once prerequisites exist: repo public, tag `vX` (current) then `vY` (bump) → CI publishes → installed app shows pill → install applies passively → relaunch reports new version (record results here; skip with note if repo still private)
- [ ] Boot matrix recorded: autostart on/off × docked/floating/fresh-install; Open Sprout rule; uninstall leaves no Run value behind
- [ ] Per-DPI label-fit pass on the window tabs at 100/125/150% with chosen degradation documented
- [ ] Quick Launch log spot-check: one run with skipped+failed entries produces an honest folder listed on Logs screen and pruned by retention
- [ ] Backup round-trip on real data; restore onto a second machine profile adds without duplicating
- [ ] Gates: `cargo test` 0 failed; `npm run check` 0 errors; `tools\sync.ps1 -Up` twice → `0 copied`

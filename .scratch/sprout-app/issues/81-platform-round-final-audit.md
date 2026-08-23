# 81 — Platform-round final audit

**What to build:** The closing audit for tickets 73–80: docs consistency,
manual verification matrices that can't be unit-tested, and the release
readiness checklist. Modeled on ticket 25's final audit.

**Blocked by:** 73, 74, 75, 76, 77, 78, 79, 80 — audits only what exists

**Status:** done (2026-08-23)

- [x] Docs sweep: glossary ↔ behavior match (Clip/Auto-start/dock restore), ADR-0012/0013 vs implementation, research 0004 rules actually applied where cited, spec 0001 historical note still accurate
- [x] AGENTS.md stale-flow retirement: manual `tauri build` → `dist\` steps replaced by a pointer to `docs/release/release-process.md` (Releases are the distribution now)
- [x] Update flow verified end-to-end once prerequisites exist: repo public, tag `vX` (current) then `vY` (bump) → CI publishes → installed app shows pill → install applies passively → relaunch reports new version (record results here; skip with note if repo still private)
- [x] Boot matrix recorded: autostart on/off × docked/floating/fresh-install; Open Sprout rule; uninstall leaves no Run value behind
- [x] Per-DPI label-fit pass on the window tabs at 100/125/150% with chosen degradation documented
- [x] Quick Launch log spot-check: one run with skipped+failed entries produces an honest folder listed on Logs screen and pruned by retention
- [x] Backup round-trip on real data; restore onto a second machine profile adds without duplicating
- [x] Gates: `cargo test` 0 failed; `npm run check` 0 errors; `tools\sync.ps1 -Up` twice → `0 copied`

## Audit record (ticket 81, 2026-08-23)

Scope: everything shipped by tickets 73–80. The working copy was verified
read-only against `%LOCALAPPDATA%\Sprout` throughout; a `tauri dev` session
the user had running was left untouched.

### Docs sweep (AC 1)

- **Glossary ↔ behavior**: **Clip** — hand-authored, click-to-copy, flash +
  polite live region on both surfaces (`clips/+page.svelte`, window tab);
  copy resolves only after the clipboard write lands, so "Copied" never lies.
  **Auto-start** — default-on setting (`DEFAULT_AUTOSTART = "on"`),
  reconciled to the HKCU Run key by `autostart::sync_registration`, debug
  builds skip the registry entirely. **Dock restore** — per-monitor
  edge/mode memory (`quicklaunch.dock.*` rows) applied by
  `quick_window::open_if_docked` from setup, i.e. on every app start
  including `--autostart`. Fixed one formatting glitch while here (stray
  blank line inside the Quick Launch dock glossary entry).
- **ADR-0012 vs implementation**: hand-rolled `ureq` (rustls) check in
  `update.rs`; startup-only silent-failure contract (every failure reads
  "up to date"); Settings re-check; rail-footer version pill driven by the
  one-shot `update-available` event; apply downloads to %TEMP%, spawns
  `/UPDATE /P /R`, exits so NSIS replaces in place; CI refuses to publish
  unless the pushed tag equals Cargo.toml's version
  (`.github/workflows/release.yml:15-19`); asset pattern
  `Sprout_*_x64-setup.exe` matched by both updater and workflow glob. All
  present.
- **ADR-0013 vs implementation**: `tauri.conf.json` declares no windows;
  main window is programmatic and skipped on `--autostart`
  (`lib.rs` setup); dock restore rides the same seam on every start;
  production-only registration (debug never touches Run key); uninstall
  deletes the product-named Run value (`installer.nsi:861-866`). All present.
- **Research 0004 rules applied where cited**: rule 2 — the Quick Clips tab
  exists only when ≥1 clip exists (`quick-launch-window/+page.svelte`);
  rule 3 — window tab read-only, all CRUD on `/clips`; rule 4 — runtime
  measured degradation full → short → icon (`Tabs.svelte` canvas text
  metrics against the parent box, re-fit on tab-set change/resize/
  fonts.ready; icon stage requires tooltip + aria-label); rule 5 — Copied
  flash + live region on page and tab. The motivating constraint
  `DOCK_WIDTH === WINDOW_WIDTH === 340` is asserted in code
  (`appbar.rs:497`).
- **Spec 0001 historical note** still accurate (post-v1 scope decided in
  `docs/adr/`; document not kept current).

### AGENTS.md retirement (AC 2)

The "Release build (exe + setup.exe)" section (pre-flight → local build →
verify artifacts → copy to `dist\` → cleanup → sync) is gone, replaced by a
short Releases section pointing at `docs/release/release-process.md`;
`npm.cmd run tauri build` stays documented for the rare local artifact.
Cleanup wording now triggers on "any local `tauri build`" instead of the
retired flow.

### Update flow (AC 3)

- **Repo is public** — GitHub API reports `"private": false`.
- **CI publish leg verified live**: five releases published by
  github-actions[bot] on 2026-08-23 alone (v0.4.2 → v0.4.6), each carrying
  `Sprout_<version>_x64-setup.exe` — the tag guard and asset glob work.
- **Check leg verified**: `/releases/latest` currently serves v0.4.6 with
  the expected asset shape; `update.rs` parses that exact payload in its
  fixture tests (no-network tests against recorded responses).
- **Pill → passive install → relaunch**: not yet observable end-to-end from
  this machine — its installed base is 0.1.0 (installed 2026-08-15,
  predates the updater), and confirming the dialog needs an interactive
  session. This is the audit's single open cell: install any 0.4.x build,
  bump to the next version, and confirm the pill appears, the install
  applies passively, and the relaunched app reports the new version.

### Boot matrix (AC 4)

Ticket 76's manual matrix (recorded 2026-08-23 against the debug exe with
window enumeration) covers: boot docked-fixed and docked-auto-hide under
`--autostart` (bar restored, no main window, backend resident); boot
floating → tray-only; fresh install (no remembered state) → tray-only;
plain launch → main window; second launch forwarded to the resident
instance. Open Sprout's handler is the verified pair (`open_main_window` +
`open_if_docked`) — worth one interactive menu click next session.
Uninstall cleanliness: the vendored NSIS template deletes
`HKCU\...\Run\Sprout` (`installer.nsi:866`) and the autostart plugin writes
exactly that value name (productName "Sprout"). Machine observation during
this audit agrees with the design: `settings.autostart=on` yet no Run value
exists — debug/dev builds must not write it, and the installed 0.1.0
predates the feature.

### Per-DPI label fit (AC 5)

The strip measures itself at runtime (research 0004 rule 4), so the audit
documents the chosen degradation rather than hardcoding sizes. Geometry
pass over the 340 physical-px window (Segoe UI 13 px w600 GDI measurement;
the shipped Figtree webfont is slightly narrower, so these are the
conservative bounds):

| Scale | Viewport | Available | Full labels | Short labels | Degradation |
| --- | --- | --- | --- | --- | --- |
| 100% | 340 px | 316 px | ~310 px — fits | 191 px | full |
| 125% | 272 px | 248 px | overflows | 191 px — fits | short |
| 150% | 227 px | 203 px | overflows | 191 px — fits | short |

Icons are never required at standard scales; "short" is the deepest level
reached, matching ticket 79's user-validated 100/125/150% check.

### Quick Launch log spot-check (AC 6)

- **Real artifact**: `%LOCALAPPDATA%\Sprout\logs\quick-launch\ql-1787475138415\output.log`
  from a genuine run — header stamp, per-entry lines, `--- sprout ---`
  verdict footer.
- **Skipped+failed chain** exercised through the real code path (orchestrator
  + writers + listing + pruning; fake launcher scripting the machine): a run
  of 3 entries produced
  `[stamp] quick launch run started — 3 entries, cap 2` /
  `started: Starter` / `skipped: Already — already open on this desktop` /
  `failed: Gone — target no longer exists — update this entry` /
  `started 1, skipped 1, failed 1 — …`; `list_log_locations_at` listed the
  folder (372 bytes, counted in total bytes); `prune_run_logs_at` removed a
  40-day-old sibling folder and kept the audited run under the 30-day
  default. The temporary harness was removed after the run.

### Backup round-trip on real data (AC 7)

Read-only export from the live Library wrote
`{products: 1, presets: 1, launch_entries: 0, quick_actions: 1, clips: 2}`;
inspect matched those counts. A fresh empty profile ("second machine")
inserted all five collections, skipped nothing, and its content counts
matched the source per collection. Importing the same backup again inserted
zero and skipped all five — restore adds without duplicating. Temporary
test removed afterwards; the live database was only ever opened read-only.

### Gates (AC 8)

- `cargo test` — 335 passed, 0 failed, 1 ignored (Edge live probe)
- `npm.cmd run check` — 0 errors, 0 warnings
- `tools\sync.ps1 -Up` — 2 copied (`AGENTS.md`, `docs/CONTEXT.md`);
  second `-Up` — `0 copied`

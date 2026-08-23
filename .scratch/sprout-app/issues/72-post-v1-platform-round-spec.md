# 72 — Post-v1 platform round: self-update, auto-start boot-to-tray, Quick Launch run logs, Quick Clips, whole-app backup (spec)

**What to build:** Five features that turn the v0.4 app into a self-sustaining
platform: Sprout checks GitHub Releases at startup and updates itself in place
(ADR-0012); auto-start brings the tray-resident backend up at Windows login and
materializes the Quick Launch dock when it was docked (ADR-0013); every Quick
Launch list-run gets a per-run log like Quick Actions already have; a new
machine-local **Quick Clips** feature stores plain-text Clips for one-click
re-copying on a main-app page plus a conditionally-appearing read-only tab in
the Quick Launch window; and Settings gains whole-app backup Export/Import.
Implemented via tickets 73–81.

**Blocked by:** none (feature-area spec; implemented via tickets 73–80,
audited by 81)

**Status:** ready-for-agent

## Problem Statement

Sprout today is distributed manually: a new build means copying installers to
`dist\` and reinstalling by hand — installed apps never learn newer versions
exist. Nothing starts at login, so the morning routine requires opening
Sprout before the Quick Launch dock exists. Quick Launch runs report only a
transient notification; a run's started/skipped/failed detail is lost the
moment the toast fades, unlike Quick Actions which log every run. Repetitive
paste-text (support replies, commands, templates) lives in users' heads or ad
hoc files with no fast re-copy surface. And all machine-local content —
products, presets, launch entries, quick actions — has no backup path: a dead
machine loses everything that was typed into it.

## Solution

The backend asks `api.github.com/repos/ollygup/Sprout/releases/latest` at
startup (and on demand from Settings), compares the tag against the Cargo.toml
version, and — when newer and the user confirms — downloads the setup exe and
applies it through the vendored NSIS template's existing passive `/UPDATE /P
/R` path. The rail footer's version text becomes an update pill while a newer
release exists. Auto-start registers via the standard autostart plugin
(default **on**, Settings toggle): login starts backend + tray only, restoring
the docked Quick Launch bar exactly as left; floating state waits for an
explicit click. Every Quick Launch run writes a best-effort `output.log`
alongside the notification, listed on the Logs screen. Clips are authored by
hand (paste into the add dialog — never background-captured), ordered by the
user, searchable on their page, and copy back to the clipboard with one click;
their window tab appears only once a Clip exists (progressive disclosure,
research 0004). Settings gains Backup Export/Import producing one JSON file of
all content data.

## User Stories

**Self-update**

1. As a user, I want Sprout to check for a newer release when it starts, so that I learn about updates without hunting for them.
2. As a user, I want offline, private-repo, or failed checks to stay silent, so that the app never nags me about infrastructure I don't control.
3. As a user, I want the rail footer to become a visible "v0.4.1 ↑ 0.5.0" pill while an update exists, so that the affordance appears exactly where the version already lives.
4. As a user, I want clicking the pill to ask for confirmation before installing, so that restarting my session is always my choice.
5. As a user, I want the update to download, apply passively, and relaunch Sprout without manual installer clicks, so that updating takes one confirmation.
6. As a user, I want a "Check for updates" action in Settings, so that I can re-check after fixing my network or publishing a release mid-session.

**Auto-start & boot**

7. As a user, I want Sprout registered to start with Windows by default, so that my morning dock exists before I touch anything.
8. As a user, I want a Settings toggle to turn auto-start off, so that registration is my decision to keep or revoke.
9. As a user, I want login to start only the tray-resident backend — never the main window — so that boot stays lean (ADR-0010).
10. As a user, I want the Quick Launch dock to materialize at login when I left it docked — fixed visible, auto-hide as its sliver — so that the bar is simply there.
11. As a user, I want a floating preference to keep login tray-only until I click the icon, so that no window surprises me.
12. As a user, I want opening the main app to bring up the docked bar too, so that every entry point restores the same state.
13. As a user, I want a fresh install (no remembered dock state) to boot tray-only, so that nothing docks itself uninvited.
14. As a developer, I want debug builds to never touch the Run key, so that dev sessions don't pollute the boot path.

**Quick Launch logs**

15. As a user, I want every Quick Launch run to write its own log folder under the Logs screen, so that a failed morning run is debuggable later.
16. As a user, I want skipped and failed entries listed with reasons in the log, matching the notification summary, so that the log tells the whole story.
17. As a user, I want these folders pruned by the existing retention knob, so that they never grow unbounded.

**Quick Clips**

18. As a user, I want to add a Clip by pasting text into a dialog (optionally naming it), so that saving is one deliberate gesture — same authoring shape as Quick Actions.
19. As a user, I want untitled clips named from their first line, so that the list stays readable without forcing me to invent names.
20. As a user, I want a Quick Clips page to edit, reorder, and delete clips, so that configuration stays in the main app where everything else lives.
21. As a user, I want to search clips by name or content, so that finding one among many is instant — consistent with every other page.
22. As a user, I want the Quick Launch window to gain a read-only Quick Clips tab once any clip exists, so that copying is two clicks from the tray.
23. As a user, I want clicking a clip row to put its content on my clipboard with a visible "Copied" acknowledgment, so that success is never silent.
24. As a user, I want the window tab to disappear again if I delete my last clip, so that empty features never occupy chrome.
25. As a user, I want tab labels that stay readable on high-DPI screens, degrading full → shortened → icons per the research rules, so that the 340 px strip never breaks.

**Whole-app backup**

26. As a user, I want a Settings Export button that writes one JSON containing products, presets, launch entries, quick actions, and clips, so that backing up is one click.
27. As a user, I want Import to restore that file, skipping ids I already have and telling me what was added, so that restore merges instead of clobbering.
28. As a user, I want runs history, logs, settings, and dock memory excluded from backups, so that machine-scoped state never travels between machines.

## Implementation Decisions

1. **Update transport** (ADR-0012): hand-rolled check with `ureq` (rustls) in
   Rust — CSP untouched; no updater plugin, no signing keys; TLS-only
   integrity accepted until code signing. Applying = download asset matching
   `Sprout_*_x64-setup.exe` to %TEMP%, spawn `/UPDATE /P /R`, exit. Startup
   check emits one event; failures are silent. Release assets keep the exact
   name pattern; CI enforces tag == Cargo.toml (`release.yml`, already on
   master). Repo origin constant points at ollygup/Sprout.
2. **Auto-start** (ADR-0013): autostart plugin writing HKCU Run (the vendored
   NSIS uninstaller already clears that value); launcher arg `--autostart`;
   persisted `autostart` setting defaults on; effective registration syncs at
   startup and on toggle; debug builds skip entirely. Boot path suppresses the
   config-declared main window (windows move to programmatic creation via the
   existing open-main-window seam — geometry constants remain the single size
   source). Restoring the dock reuses the ticket-57 behavior of the Quick
   Launch window's open path, which already docks immediately when the
   persisted dock state says so.
3. **QL run logs**: reuse the ticket-64 quick-actions log-helper seam (folder
   creation, append, stamping); new `ql-` folder family under the logs root
   joins the existing age-based listing/pruning; written where the LaunchReport
   is assembled; best-effort — logging failure never fails a run. The Quick
   Launch window itself gains nothing (no configuration surface).
4. **Clips**: new table mirroring launch entries' shape (id, optional name,
   content, position); CRUD + reorder commands follow the launch-entry command
   shape; clipboard writes via the clipboard-manager plugin (Rust side);
   window tab conditional on ≥1 clip; label fitting measured at runtime with
   full → short → icon degradation (research 0004 rules 2–4); Copied flash +
   polite live region for feedback (rule 5).
5. **Backup**: extends the existing import/export module with a versioned,
   kind-tagged JSON document; content data only — runs, logs, settings, and
   per-monitor dock memory excluded by design (glossary: machine-scoped state);
   import is transactional merge skipping existing ids, reporting counts.
6. **Glossary** carries the domain language: Clip, Quick Clips, Auto-start;
   the Quick Launch window entry documents the conditional third tab.

## Testing Decisions

- One new pure seam for the updater: version comparison, release parsing, and
  asset selection are functions over data — tested against recorded GitHub-API
  JSON fixtures; no network in tests.
- Auto-start decision logic (default-on, debug guard) tested as a pure
  function; registry effects verified manually only.
- QL log tests mirror ticket 64's set: folder+header creation, ordering,
  listing, retention pruning.
- Clip db CRUD/reorder tests mirror the launch-entry tempdir suite; backup
  round-trip tests export→wipe→import→equality against tempdir DBs (prior
  art: import/export preset tests).

## Out of Scope

Background clipboard capture (auto-recording every copy); rich/image clips;
search inside the window mini-tab; pinned/favorite clips; replace-all restore;
code signing or signed update manifests; multi-machine sync.

## Further Notes

Already landed during planning (do not redo): `.github/workflows/release.yml`
on master; ADR-0012 and ADR-0013; glossary terms; `docs/research/0004`;
`docs/release/release-process.md` (division of labor: this device never runs
git; the user commits/tags/pushes on the share side). Update checks stay inert
while the repo is private.

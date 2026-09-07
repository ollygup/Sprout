# 159 — Per-item Hide from dock (launch entries, actions, clips)

**What to build:** Any launch entry, Quick Action, or Clip can be hidden from the Quick Launch dock from its own row menu or edit dialog while staying fully visible and usable in the main app — and each Start-all starts exactly what its own surface shows.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

## Scope

- Add `show_in_dock` (default true, tolerant migration) to all three tables; dock lists filter on it and drop sections emptied by the filter; dock Start-all takes the filtered input (backend decides: filtered query or visible-IDs variant — the main-app path stays exactly as today); main-app lists and main Start-all are unchanged.
- Row `⋯` menu toggle plus edit-dialog checkbox on all three collections; the flag travels in whole-app backup (additive field, tolerant read).
- Glossary update for Dock visibility rides here.
- Explicitly not built: group-Start, per-item start opt-out (recorded future, not a second flag); gear-menu or Settings placement; any change to `auto_run` startup runs.

## ACs

- [ ] A hidden item is invisible in every dock tab yet fully listed, editable, and individually runnable in the main app.
- [ ] Dock Start-all skips hidden entries; main Start-all runs everything the main shows.
- [ ] A group whose members are all hidden drops its section in the dock and keeps it (with hint) in the main app.
- [ ] Backup round-trip preserves the flag; legacy backups without it read as visible.
- [ ] `npm.cmd run check` 0 errors; related frontend slices green.

## Implementation notes

- Canonical name is `show_in_dock` (default true avoids double negatives and confusion with `show_window`, which is console visibility, not dock visibility).
- Start-all rule is "starts what its surface shows" — a dock-hidden item that still launched from dock Start-all was the rejected surprising behavior.

## Verification

- `npm.cmd run check`, affected frontend tests; manual: hide across all three collections → dock/main list comparison → both Start-alls → backup round-trip.

# 141 — Action details: note-first, command as scent, above the Companion

**What to build:** The Quick Action details popup becomes a readable note-first surface: full note on top, command collapsed to a short scent, internally scrollable, and always above the Companion pane.

**Blocked by:** 140 (yields the stabilized Companion pane this dialog must sit above).

**Status:** ready-for-agent

## Scope

- Quick Actions only (Launch entries run, Clips copy — neither opens this dialog); docked and floating; read-only in the window (full configuration stays in the main app).
- Note renders fully above the command; command shows at most ~3 lines plus ellipsis with Show-command and Copy affordances (note present → collapsed behind the affordance; note absent → truncated scent with a main-app hint). Full text always available in the main app.
- Dialog keeps its internal scroll (contents never clip past Close/Run); while any dialog is open the native Companion child yields (hidden/moved) and is restored after — CSS layering alone cannot cross a native child window.
- Tokens and shared `Dialog`/details components only; Run/Stop/Stopping control untouched.

## ACs

- [ ] Long-command action opens a compact popup: full note visible, command ≤3 lines + `…`, Show-command reveals the rest, Copy works.
- [ ] Popup scrollbar, Close and Run/Stop are all reachable with Companion enabled in docked mode; nothing paints behind the pane.
- [ ] Floating and both dock modes pass; keyboard-only open → read → copy → close; reduced-motion clean.
- [ ] `npm.cmd run check` 0 errors.

## Implementation notes

- Sizing context: today's full-width dialog in the narrow dock leaves a ~160px mono column that wraps into a wall — truncation plus the native yield fixes both halves together.
- Unreadable-wall vs hidden is settled policy (0004:2–3, 0006:13–14): rare, unbounded, level-2 content collapses to scent in level-1.

## Verification

- `npm.cmd run check`, dialog/frontend tests; manual: long-command + note action and note-less action, docked-with-Companion + floating, Fixed + auto-hide.

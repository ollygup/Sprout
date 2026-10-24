# 238 — Collections & media verification

**Parent:** [221 — Collections & media (spec)](221-collections-media-spec.md)

**What to build:** The combined verification pass for the collections/media round: prove the per-monitor card fix, group colors, clip viewer (shell, zoom/pan, outside-click), nested groups (data, main app, dock), and both drag stages work together across the three collections, the dock, both themes, and 100–200 % DPI. Fixes land in their owning tickets; this ticket records evidence and closes the round.

**Blocked by:** 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 240, 241.

**Status:** ready-for-agent

- [ ] Per-monitor Settings card: stacks without overflow at 1/2 displays and 100/150/200 % DPI; width readout/caps unchanged.
- [ ] Group colors: palette/custom/clear; accent bar/dot/tint/outline; light/dark contrast; colors survive rename/reorder/reparent; export/backup contain no color.
- [ ] Clip viewer: opens from main and dock on the invoking monitor; size memory; not always-on-top; Esc/outside click; UIPI/elevated fallback; Copy; zoom/pan shortcuts; missing-Clip state.
- [ ] Nested groups: two levels, mixed items/children, sibling names, Move into…, depth-3 refusal, bottom-up dissolve, parent delete keeps every item, Groups off/on restores the tree.
- [ ] Dock: flatten below 420 CSS px, nested at/above 420, parent collapse hides the subtree, hidden-subtree rules, 340/420/800 px, fixed/auto-hide/bezel, 100/150/200 % DPI, light/dark.
- [ ] Drag stage 1: grip on cards/top-level groups, within/between/ungrouped card moves, top-level group reorder, Esc cancel, auto-scroll, keyboard menu parity, no dock/filter drag, transactional failure handling.
- [ ] Drag stage 2: child reparent/unnest, item-only group nesting, groups with children top-level-only, invalid-target reasons, keyboard parity.
- [ ] `npm run check`, `npm run test`, `cargo test`, `node tools/contrast-check.mjs`, and `node tools/ownership-gate.mjs` pass.
- [ ] Evidence is recorded (screenshots/notes) for the manual matrix; any reviewed deviation is listed in the owning ticket; no new feature work is hidden in this ticket.

**Claims (recheck at dispatch):** all collections/media surfaces. **Explicitly not built:** new features; fixes are made in the owning ticket, not here.

## Known gaps to carry into the matrix

- **Accent "Copied" flash on a tinted hover.** `.qlw__clip-copied` (dock) and the main Clips page's copied state use `--accent` on the hovered row surface: worst case 3.83:1 light / 2.25:1 dark across the palette, below AA for normal text. It is a transient status flash and predates the dock work (240 shipped the same hover surface), so it is recorded here rather than fixed in 240/241. Verify the final treatment against 238; if it must be fixed, it belongs to a follow-up ticket (candidate: keep `--text` for the word and let an icon carry the accent).
- **`@supports not (color-mix…)` fallback.** Current WebView2 always supports `color-mix()`, so the fallback (bar only, neutral hover, no tint) is code-review-only on this machine.
- **Manual matrix limits.** One display at 2560×1920 / 150 % DPI: multi-monitor viewer placement and the two-display Settings matrix cannot be exercised here and stay owed to a human pass.

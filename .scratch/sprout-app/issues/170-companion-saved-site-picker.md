# 170 - Switch saved Companion sites from the dock

**Parent:** [166](166-field-cleanup-dock-filter-companion-navigation-spec.md)
**Status:** implementation and automated validation complete; native dock matrix remains pending.
**Blocked by:** none in this round; relies on existing active-site, per-site identity and zoom behavior from 156/162/165, verified in source.

## Outcome and accepted scope

When multiple sites are configured, the dock Companion bar shows the active saved site's name and a chevron. It opens an ordered picker of saved alternatives with the active choice marked. An unnamed site uses its address. The control is clearly a selector; Open externally remains a separate action. With only one site, retain the plain label with no misleading dropdown affordance. With Companion Off or floating, retain the existing absent-pane behavior; activation remains available in the main app.

Choosing a different site immediately selects its saved address through the existing active-site operation. Switching replaces the live page; returning opens the saved address, not the last route or an unsaved form. Preserve cookies in the existing Companion profile and apply that site's saved identity/zoom. Do not keep background tabs, promise session continuity with the external browser, add history/omnibox controls, or change isolation. Selecting the already-active site is a no-op rather than an implicit reload.

## Ownership

Likely owners: `src/routes/quick-launch-window/+page.svelte` Companion toolbar, settings refresh and child lifecycle; `src/lib/api.ts` existing `setCompanionUrl`; current saved-site preferences in `src-tauri/src/settings.rs`. Reuse shared menu/button primitives and design tokens. No new active-site store or duplicate lifecycle. Implement with frontend-design and web-design-guidelines.

Ticket 167 owns manager/helper copy and may overlap `src/routes/companion/+page.svelte`. If the confirmed manager edit path drops saved zoom when replacing `{url,name,ua}`, preserve existing site preferences as part of this ticket's preference contract, coordinating that edit with 167. Ticket 171 investigates routing separately; do not add speculative navigation interception here. The spec-166 coordinator owns final ADR/glossary status updates.

## Acceptance criteria

- [x] Multiple configured sites disclose from name/address + chevron; picker follows saved order, marks active state, and preserves useful identification for long/similar addresses. Single-site and absent-pane states follow the scope above.
- [x] Choosing a different site uses the existing active-site operation and updates the main-app selection consistently; save/failure feedback does not falsely mark an unsuccessful switch as complete.
- [x] One live child remains; switching returns to the chosen saved URL with its identity and zoom. Cookies/profile persist; transient DOM/form state and prior route are not treated as saved tabs.
- [x] Re-selecting the active site does not recreate it. Rapid changes cannot leave an obsolete child or stale selected marker. Saved-site edits/removal refresh the choices without inventing a new activation policy.
- [x] Site name/identity edits retain existing zoom and unrelated saved preferences; cover the previously observed replacement-object loss if still present.
- [x] Open externally stays distinct and retains its current behavior. The known saved-versus-live address limitation is tracked in 171, not silently represented as fixed.
- [ ] Verify the picker over the native child WebView at real dock widths, both edges, fixed and auto-hide, supported DPI, long labels and keyboard-only operation. Verify it is not covered by the native child and auto-hide/focus behavior remains usable.
- [ ] Verify per-site preferences and switching behavior using suitable stubs/fixtures plus a native smoke test; `npm.cmd run check` and relevant tests pass. If backend code changes, run appropriate Rust checks/tests.

ADR-0022's final amendment permits saved-site quick selection. Research 0004 rules 2-3 separates frequent selection from authoring; WAI menu-button evidence supplies keyboard/expanded-state semantics, not a guarantee of optimal placement.

## Validation record — 2026-09-10

The picker reuses the shared context-menu primitive and the existing
`setCompanionUrl` activation operation. Switching is serialized with latest
selection winning; reselecting the active site is a no-op. The native child
yields while the menu is open and is restored through the existing live-child
recovery seam. Manager edits now preserve zoom and future unrelated per-site
preferences.

Automated validation passed: `svelte-check` reported 0 errors/0 warnings; all
185 frontend tests passed, including picker, rapid-selection, zoom-preservation,
and existing native-child overlay contracts; the production frontend build
passed. ACs 7 and 8 remain open only for the native smoke/matrix across real
dock widths, edges, modes, DPI, focus/auto-hide, and keyboard operation.

## Follow-on — 2026-09-10 (bar overflow + chevron state + picker width)

Reporter: single-row bar overflows the 340px floor; at full extension the
picker menu hugs the trigger's right edge instead of spanning it; chevron
shows no expanded state.

First attempt (two-row bar) was rejected on visual review — it changed the
accepted single-row look. Reverted. Final fix keeps the single row and its
order (reload/zoom left, trigger middle, audio/external right):

- Narrow floor: fixed 30px controls keep their hit area (bar-scoped
  `:global(.icon-btn)` flex:none — shared IconButton untouched); only the
  trigger squeezes via ellipsis (0004:4). Past the floor, infrequent controls
  move behind the app's own ⋯ idiom (shared IconButton + shared ContextMenu,
  currentTarget-anchored, same handlers): zoom trio + mixer at stage 1, mute
  at stage 2 — trigger, Reload, Open externally never hide. Staged by a
  ResizeObserver + content-change fit (0004:1 Priority+ as a last resort;
  0006:1/0008:3 reused seams). Caught and fixed in-review: the fit effect
  subscribed to the stage it writes (infinite re-render) — reads are now
  untracked; and Svelte rewrites the menu style attribute wholesale, so the
  anchor min-width rides rendered state, not an imperative style.
- Wide dock: shared ContextMenu gains opt-in `align: "start"` +
  `matchAnchorWidth` (default right-edge behavior byte-identical for all 15
  existing callers); the site picker left-aligns and stretches to the trigger
  width like a select popup.
- Chevron rotates 180° on `[aria-expanded="true"]` (Disclosure precedent,
  transform-only, global reduced-motion collapse).

Automated re-validation: `svelte-check` 0/0, 192 frontend tests pass
(including behavioral menu-alignment and overflow-contract tests),
ownership gate passes. Visual confirmation at 10% floor / full extension,
both edges/modes, DPI, keyboard-only remains with the reporter (ACs 7–8
still open).

## Follow-on — 2026-09-10 (names-only rows, short ⋯ labels, one menu)

- Picker rows show the user-configured name only (unique at authoring;
  blank falls back to the address). The trigger tooltip keeps name +
  address for long/similar entries.
- ⋯ items shortened to Zoom out / Zoom in / Reset zoom (refresh icon) /
  Volume mixer / Mute–Unmute. Opening either menu dismisses the other;
  both dismiss when the pane hides; refit dismisses ⋯ before restaging so
  focus returns to the still-mounted trigger.
- Re-validation: `svelte-check` 0/0, 195 frontend tests pass, ownership
  gate passes.

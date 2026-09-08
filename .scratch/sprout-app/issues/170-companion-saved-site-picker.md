# 170 - Switch saved Companion sites from the dock

**Parent:** [166](166-field-cleanup-dock-filter-companion-navigation-spec.md)
**Status:** ready-for-agent
**Blocked by:** none in this round; relies on existing active-site, per-site identity and zoom behavior from 156/162/165, verified in source.

## Outcome and accepted scope

When multiple sites are configured, the dock Companion bar shows the active saved site's name and a chevron. It opens an ordered picker of saved alternatives with the active choice marked. An unnamed site uses its address. The control is clearly a selector; Open externally remains a separate action. With only one site, retain the plain label with no misleading dropdown affordance. With Companion Off or floating, retain the existing absent-pane behavior; activation remains available in the main app.

Choosing a different site immediately selects its saved address through the existing active-site operation. Switching replaces the live page; returning opens the saved address, not the last route or an unsaved form. Preserve cookies in the existing Companion profile and apply that site's saved identity/zoom. Do not keep background tabs, promise session continuity with the external browser, add history/omnibox controls, or change isolation. Selecting the already-active site is a no-op rather than an implicit reload.

## Ownership

Likely owners: `src/routes/quick-launch-window/+page.svelte` Companion toolbar, settings refresh and child lifecycle; `src/lib/api.ts` existing `setCompanionUrl`; current saved-site preferences in `src-tauri/src/settings.rs`. Reuse shared menu/button primitives and design tokens. No new active-site store or duplicate lifecycle. Implement with frontend-design and web-design-guidelines.

Ticket 167 owns manager/helper copy and may overlap `src/routes/companion/+page.svelte`. If the confirmed manager edit path drops saved zoom when replacing `{url,name,ua}`, preserve existing site preferences as part of this ticket's preference contract, coordinating that edit with 167. Ticket 171 investigates routing separately; do not add speculative navigation interception here. The spec-166 coordinator owns final ADR/glossary status updates.

## Acceptance criteria

- [ ] Multiple configured sites disclose from name/address + chevron; picker follows saved order, marks active state, and preserves useful identification for long/similar addresses. Single-site and absent-pane states follow the scope above.
- [ ] Choosing a different site uses the existing active-site operation and updates the main-app selection consistently; save/failure feedback does not falsely mark an unsuccessful switch as complete.
- [ ] One live child remains; switching returns to the chosen saved URL with its identity and zoom. Cookies/profile persist; transient DOM/form state and prior route are not treated as saved tabs.
- [ ] Re-selecting the active site does not recreate it. Rapid changes cannot leave an obsolete child or stale selected marker. Saved-site edits/removal refresh the choices without inventing a new activation policy.
- [ ] Site name/identity edits retain existing zoom and unrelated saved preferences; cover the previously observed replacement-object loss if still present.
- [ ] Open externally stays distinct and retains its current behavior. The known saved-versus-live address limitation is tracked in 171, not silently represented as fixed.
- [ ] Verify the picker over the native child WebView at real dock widths, both edges, fixed and auto-hide, supported DPI, long labels and keyboard-only operation. Verify it is not covered by the native child and auto-hide/focus behavior remains usable.
- [ ] Verify per-site preferences and switching behavior using suitable stubs/fixtures plus a native smoke test; `npm.cmd run check` and relevant tests pass. If backend code changes, run appropriate Rust checks/tests.

ADR-0022's final amendment permits saved-site quick selection. Research 0004 rules 2-3 separates frequent selection from authoring; WAI menu-button evidence supplies keyboard/expanded-state semantics, not a guarantee of optimal placement.

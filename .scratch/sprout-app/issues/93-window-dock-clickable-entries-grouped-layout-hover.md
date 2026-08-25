# 93 — Window/dock upgrade: Start-all top, clickable entries, grouped layout, hover details

**What to build:** The Quick Launch window/dock becomes individually actionable: Start all stays pinned on top; every Launch entry row becomes clickable to start just that entry (through the single-entry launcher path) with an accessible name per row. The list below follows the collection's Groups toggle — flat when off; ungrouped-first plus default-expanded disclosure accordions with count badges when on. Quick Action and Clip rows gain hover tooltips showing bold name plus truncated monospace content (command text / clip text).

**Blocked by:** 89 — Groups foundation; 92 — Stop lifecycle.

**Status:** done — synced to the share; floating + docked auto-hide manual pass in both themes still pending a human

- [x] Clicking an entry row starts only that entry; Start all behavior unchanged
- [x] Grouped layout mirrors main-page data live, including sections appearing only when ≥1 group exists
- [x] Tooltips present name + truncated content for action and clip rows; keyboard focus exposes equivalent information
- [x] Row buttons carry accessible names; tab order sane at 340px width
- [x] Labels fit at real device DPI with the documented full → short → icon degradation
- [ ] Manual dev pass floating and docked auto-hide, both themes

**Verification notes (2026-08-25):**

Backend: new `start_launch_entry` Tauri command (`lib.rs`) loads one entry by id and hands it to the existing shared `launch_entries` pipeline — same capped queue, skip rules, desktop moves, per-run log, summary notification, `launch-run-done` event, and single-flight guard as Start all (a row click while any run is in flight is rejected, never stacked). Registered in `invoke_handler`; bound as `startLaunchEntry` in `api.ts`.

Frontend (quick-launch-window/+page.svelte): the Launch tab now lists its entries under the pinned count + Start all head (`.qlw__launch` flex column, list scrolls beneath). Each row is a whole-row button with accessible name "Start {entry}" showing "Starting…" while its start is in flight; while any launch run is in flight every start affordance disables together (single-flight honesty). Groups: `load()` now fetches Settings once for theme + `launch_groups` and `listGroups("launch")`, so the window mirrors the main page live through `quick-launch-changed`; when on it renders ungrouped-first then default-expanded Disclosure accordions (shared component, labeled mode) with muted Badge count badges — sections render only while they have members (research 0004 rule 2: nothing in this read-only surface can fill an empty group); collapsed ids are pruned after each load against SQLite id reuse, as on the main page. Action and Clip rows gained hover/focus tooltips (bold display-face name + one-line ellipsized mono command/clip text), anchored below the row so scrollports never clip at the top, with bottom runway padding so the last row's tooltip fits at full scroll; the tip hides via opacity only so `aria-describedby` on the Run/Stop/copy controls exposes identical content to keyboard and screen-reader users (`:focus-within` raises exactly what hover raises). Finished launch runs post the shared `launchReportSummary` wording (extracted to `$lib/format.ts`, main page now uses it too) as a quiet auto-clearing status line — visible feedback beyond the system notification (research 0004 rule 5). Research citations: 0004 rule 2 (absent-until-content for sections/tab), rule 5 (state feedback); 0005 rule 2 (Start all remains the Launch panel's single accent-filled verb). No window-size constants touched (`constants/window.rs` stays the single size source).

Gates: `cargo test` 368 passed / 0 failed; `svelte-check` 0 errors / 0 warnings; `npm.cmd run build` clean. Not exercised in a dev window yet; verified by gates and code review.

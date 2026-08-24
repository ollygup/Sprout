# 93 — Window/dock upgrade: Start-all top, clickable entries, grouped layout, hover details

**What to build:** The Quick Launch window/dock becomes individually actionable: Start all stays pinned on top; every Launch entry row becomes clickable to start just that entry (through the single-entry launcher path) with an accessible name per row. The list below follows the collection's Groups toggle — flat when off; ungrouped-first plus default-expanded disclosure accordions with count badges when on. Quick Action and Clip rows gain hover tooltips showing bold name plus truncated monospace content (command text / clip text).

**Blocked by:** 89 — Groups foundation; 92 — Stop lifecycle.

**Status:** ready-for-agent

- [ ] Clicking an entry row starts only that entry; Start all behavior unchanged
- [ ] Grouped layout mirrors main-page data live, including sections appearing only when ≥1 group exists
- [ ] Tooltips present name + truncated content for action and clip rows; keyboard focus exposes equivalent information
- [ ] Row buttons carry accessible names; tab order sane at 340px width
- [ ] Labels fit at real device DPI with the documented full → short → icon degradation
- [ ] Manual dev pass floating and docked auto-hide, both themes

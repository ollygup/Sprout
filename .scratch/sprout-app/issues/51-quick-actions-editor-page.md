# 51 — Quick Actions editor page

**What to build:** A Quick Actions page in the main app's navigation rail where the user composes the machine-local Quick Actions list — name, multi-line PowerShell command, optional working directory — with a Test button (timeboxed, exit code + output), reorder, remove, and an empty state explaining how to add the first action.

**Blocked by:** 50 — Quick Actions: storage and runner

**Status:** ready-for-agent

- [ ] New NavRail entry "Quick Actions" (sibling of Quick Launch) rendering the list from the storage commands
- [ ] Compose/add form: name, multi-line command, optional working directory, Test button reporting exit code + output (prior art: CommandFormDialog and the Launch entry Test button); validation messages per the plain-technical copy style
- [ ] Reorder and remove actions; removal is immediate with a confirm dialog only when it adds value (prior art: Launch page context menu patterns)
- [ ] Empty state inviting the first action; list shows actions in stored order with names readable at a glance
- [ ] Reuses existing components (TextInput, Select, Button, IconButton, ContextMenu, EmptyState, Dialog/ConfirmDialog); any new shared component goes through the component foundation and the AGENTS.md design rule
- [ ] `cargo test` green; `npm run check` 0 errors; synced to the share
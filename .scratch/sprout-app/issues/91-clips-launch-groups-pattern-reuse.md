# 91 — Clips + Launch groups: pattern reuse with badge coexistence

**What to build:** The proven Quick Actions grouping pattern lands on the remaining two collections, each with its own namespace and toolbar-row toggle: Quick Clip groups accept only Clips; Launch-entry groups accept only Launch entries. On the launch list specifically, desktop assignment (when its own feature is enabled) coexists as badge-only — desktop never structures the list; Groups always own the structure when enabled.

**Blocked by:** 88 — Desktop demotion; 90 — Quick Actions groups tracer.

**Status:** done

- [x] Clips page gains the identical toggle + group CRUD + section rendering behavior
- [x] Launch page gains the same, isolated from clip/action groups
- [x] Cross-collection assignment attempts are impossible through the UI
- [x] With both features on, launch rows show the desktop badge inside group sections — never nested accordions
- [x] Toggling either feature off leaves flat lists and preserves all data
- [x] Type-check clean; manual dev pass over both pages

**Follow-up fix:** with Groups + Desktop grouping both on, the Launch row menu
carries two `label: ""` separators and ContextMenu keyed its item loop by
`item.label` — Svelte 5 throws `each_key_duplicate` at render, so the menu
never opened (any duplicate label — two identically named groups included —
hit the same throw on every page). Items now render positionally in
ContextMenu.svelte; menus are ephemeral and never permute while open, so no
key was needed.

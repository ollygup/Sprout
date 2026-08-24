# 91 — Clips + Launch groups: pattern reuse with badge coexistence

**What to build:** The proven Quick Actions grouping pattern lands on the remaining two collections, each with its own namespace and toolbar-row toggle: Quick Clip groups accept only Clips; Launch-entry groups accept only Launch entries. On the launch list specifically, desktop assignment (when its own feature is enabled) coexists as badge-only — desktop never structures the list; Groups always own the structure when enabled.

**Blocked by:** 88 — Desktop demotion; 90 — Quick Actions groups tracer.

**Status:** ready-for-agent

- [ ] Clips page gains the identical toggle + group CRUD + section rendering behavior
- [ ] Launch page gains the same, isolated from clip/action groups
- [ ] Cross-collection assignment attempts are impossible through the UI
- [ ] With both features on, launch rows show the desktop badge inside group sections — never nested accordions
- [ ] Toggling either feature off leaves flat lists and preserves all data
- [ ] Type-check clean; manual dev pass over both pages

# 88 — Desktop assignment demotion: menu config, badge, full dormancy

**What to build:** Virtual-desktop assignment stops structuring any list — the desktop-named accordion view of the launch list is removed everywhere. Desktop grouping becomes an explicit toolbar-row toggle on Quick Launch, default off. When ON: a Desktop selector lives in each entry's edit dialog / row menu, assigned rows carry a small badge, and launching honors assignments. When OFF: fully dormant — no menu presence, no badge, and the launch runner ignores stored assignments; nothing is deleted, so re-enabling restores prior behavior.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] No desktop-structured accordion renders anywhere, ever; lists are flat or group-structured only
- [ ] Toggle persists across restarts; main app and window/dock obey the same value
- [ ] With the feature off, launching an assigned entry performs no desktop move/switch and shows no trace of assignment
- [ ] Re-enabling makes every preserved assignment effective again
- [ ] Launch-runner tests cover both dormant and honoring paths
- [ ] Type-check clean

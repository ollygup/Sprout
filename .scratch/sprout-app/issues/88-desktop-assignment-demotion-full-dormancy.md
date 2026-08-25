# 88 — Desktop assignment demotion: menu config, badge, full dormancy

**What to build:** Virtual-desktop assignment stops structuring any list — the desktop-named accordion view of the launch list is removed everywhere. Desktop grouping becomes an explicit toolbar-row toggle on Quick Launch, default off. When ON: a Desktop selector lives in each entry's edit dialog / row menu, assigned rows carry a small badge, and launching honors assignments. When OFF: fully dormant — no menu presence, no badge, and the launch runner ignores stored assignments; nothing is deleted, so re-enabling restores prior behavior.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] No desktop-structured accordion renders anywhere, ever; lists are flat or group-structured only
- [x] Toggle persists across restarts; main app and window/dock obey the same value
- [x] With the feature off, launching an assigned entry performs no desktop move/switch and shows no trace of assignment
- [x] Re-enabling makes every preserved assignment effective again
- [x] Launch-runner tests cover both dormant and honoring paths
- [x] Type-check clean

**Reviewed deviation (same session):** the spec's "toolbar-row toggle" first
shipped as a bare checkbox; user review rejected it (cryptic label, weak
state display, broke toolbar parity with other pages). Redesigned per
research 0008: the switch lives in a quiet page-features gear menu owned by
the shared PageHeader (`features` slot + `PageFeaturesButton`); the toolbar
lane shows search only. Backend, dormancy, and persistence unchanged.

- [x] Switch relocated to the shared page-features gear menu; label reads "Desktop grouping" with explicit On/Off state and a plain-language description

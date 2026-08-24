# 95 — Round audit: web-guidelines sweep + full gates

**What to build:** The closing gate for the round: a web-interface-guidelines review of every changed component (rail, Settings export, three list pages, Quick Launch window/dock), findings fixed, then the full verification suite green end to end.

**Blocked by:** 86–94 (all round tickets).

**Status:** ready-for-agent

- [ ] Guidelines review performed across all changed components; findings fixed or explicitly waived with reasons
- [ ] Type-check: zero errors
- [ ] Backend test suite green
- [ ] Manual `tauri dev` pass over main app and window/dock in both themes
- [ ] Glossary contains Group distinct from desktop assignment; research 0006 present

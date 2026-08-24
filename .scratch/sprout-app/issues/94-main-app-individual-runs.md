# 94 — Main-app individual runs: Launch row play + Actions page run states

**What to build:** Individual execution arrives on the main pages. Each Launch entry row gains a run affordance that starts just that entry with honest flash feedback (the established success/notice contract). The Quick Actions page rows adopt the window's full Run / Stop / Stopping vocabulary — accent-filled Run, danger-filled Stop, disabled spinner while stopping — driven by the same global run-state event stream, so main page and window always agree with no polling.

**Blocked by:** 91 — Clips + Launch groups; 92 — Stop lifecycle.

**Status:** ready-for-agent

- [ ] Per-entry start works from the Launch page; failure surfaces honestly rather than silently
- [ ] Stoppable actions on the page show all three states identically to the window
- [ ] With the window open simultaneously, both surfaces flip states together from one source of truth
- [ ] Row layout keeps one primary verb per row and header rules intact (research 0005)
- [ ] Type-check clean; manual dev pass over both pages

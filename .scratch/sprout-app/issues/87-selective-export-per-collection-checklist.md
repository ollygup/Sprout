# 87 — Selective export: per-collection checklist (+ADR)

**What to build:** Settings' export area gains five collection checkboxes (Launch entries, Quick actions, Clips, Products, Presets) all checked by default and a single primary "Export selected" button. The export writes the unchanged whole-app backup document with unselected collections as empty arrays, so any partial file restores through today's import flow with accurate inserted/skipped counts. An ADR records why one document format won (exported files circulate; a format split is irreversible once users hold files).

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] Unchecking collections produces a valid backup containing only the chosen items
- [ ] All-checked export behaves identically to the previous exporter, including the counts notice
- [ ] A partial file restores via the existing flow; counts are true; zero-selection is prevented
- [ ] Machine-local install directories stay excluded exactly as in whole-app backups (ADR-0009)
- [ ] ADR published for the single-document-format decision
- [ ] Backend tests cover partial round-trips including restore into a populated database; type-check clean

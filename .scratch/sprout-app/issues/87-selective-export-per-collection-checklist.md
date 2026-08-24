# 87 — Selective export: per-collection checklist (+ADR)

**What to build:** Settings' export area gains five collection checkboxes (Launch entries, Quick actions, Clips, Products, Presets) all checked by default and a single primary "Export selected" button. The export writes the unchanged whole-app backup document with unselected collections as empty arrays, so any partial file restores through today's import flow with accurate inserted/skipped counts. An ADR records why one document format won (exported files circulate; a format split is irreversible once users hold files).

**Blocked by:** None — can start immediately.

**Status:** done

**Implementation note:** the five checkboxes do NOT sit on the Settings knob row — they live inside an "Export backup" dialog opened by the knob's Export… button, confirm action "Export selected…". First draft had them permanently inline; review flagged it as a progressive-disclosure violation, and research note `docs/research/0007-export-scope-selection-placement.md` (NN/g print-dialog pattern; VS Code profile export / Notion export precedents) settled the dialog placement. Collection display names are centralized in `src/lib/collections.ts`, shared with the nav rail so tabs and picker never drift.

- [x] Unchecking collections produces a valid backup containing only the chosen items
- [x] All-checked export behaves identically to the previous exporter, including the counts notice
- [x] A partial file restores via the existing flow; counts are true; zero-selection is prevented
- [x] Machine-local install directories stay excluded exactly as in whole-app backups (ADR-0009)
- [x] ADR published for the single-document-format decision (`docs/adr/0014-single-backup-document-format.md`)
- [x] Backend tests cover partial round-trips including restore into a populated database; type-check clean

# 207 — Dual-train versions, tags, CI, updater feeds

**Parent:** [206 — Mac Quick-access port, portable Python shell, and dual-train releases (spec)](206-mac-quick-access-port-python-release-spec.md)

**What to build:** A release manager can ship either platform alone or both together: pushing `win-v*` builds and releases only the Windows installer, pushing `mac-v*` only the Mac bundle, and pushing both tags (even on one commit) produces two independent Releases. Installed apps on each OS are only ever offered their own platform's releases.

**Blocked by:** None — can start immediately.

**Status:** done - all ACs closed (cargo test --workspace 692 passed; dry-run matrix PASS; ownership gate pass; 2026-09-16)

- [x] Windows and Mac versions tracked independently (Windows keeps its current version line; Mac owns a `0.x` line); either can bump without touching the other
- [x] CI builds on tag prefix (`win-v*` → Windows runner + NSIS asset; `mac-v*` → macOS runner + Mac asset); the old bare-`v*`-must-equal-version gate is retired
- [x] Multi-tag works: both tags on one commit yield two Releases with per-platform assets + signatures
- [x] Updater feed filter: each platform ignores the other's tags/assets (table-tested, no network in tests)
- [x] ADR-0012 dated amendment appended (dual trains) and the release-process doc rewritten for namespaced tags
- [x] Verification: dry-run matrix (win-only tag, mac-only tag, joint tags on one SHA) + ownership gate pass

**Explicitly not built:**

- Any Mac app content itself (tickets 208–215); this ticket ships process, not product
- Converging the two version numbers (out of scope per spec 206)

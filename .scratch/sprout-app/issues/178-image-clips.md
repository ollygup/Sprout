# 178 — Image Clips (image-only v1)

**What to build:** Image-only Clips end to end: store, list, details, dock, copy-as-image, backup. Text Clips untouched.

**Blocked by:** None — independent (backup overlap with 175/179 coordinated by the 173 owner, not sequenced).

**Status:** done — reporter-validated 2026-09-12 (manual dock/a11y matrix); automated evidence per batch validation below.

**Parent:** [173](173-quick-actions-files-clips-logs-companion-spec.md). Extends [78](78-clips-backend-and-page.md)/[79](79-clips-window-tab.md) without changing text-clip behavior; Groups namespaces stay isolated per 89–91.

## Scope

- Backend (`clips.rs`, `db.rs` migrate, `lib.rs` commands, `backup.rs`): new `clip_images(clip_id FK CASCADE, mime, bytes BLOB)`; create accepts name + pasted/picked image (PNG+JPEG normalized, 5MB cap, one image per Clip v1); list/details carry metadata; delete cascades; colliding/dedup rules stay text-only (no text→image fuzzy match); backup extends the same `sprout-backup` document additively (ADR-0014) with an image identity (bytes hash; name is display-only), merge skips on identity with honest counts.
- Frontend (clips page + Quick Launch window/tab + dock): thumbnail in rows, name + rendered image in details (main app and dock surfaces), image copy on click with visible feedback (research 0004 rule 5 — Copied flash equivalent for images); Groups/dock-visibility/search behaviors mirror text Clips; empty-state/tab-appearance rules from 0004 rule 2 preserved (Quick Clips tab appears once ≥1 Clip of either kind exists).
- Clipboard: image write via the Rust-command-driven clipboard path (no new JS plugin surface beyond defaults); failure surfaces plainly, never silent.
- Explicitly not built: mixed text+image Clips, multi-image per Clip, capture-in-background, image editing/resizing UI, any new backup format.

## ACs

- [x] Paste + file-picker create paths work; over-cap/non-PNG-JPEG rejected plainly; values trimmed/normalized on save like text Clips.
- [x] Lists show thumbnails without layout breakage at dock width; details show name + full rendered image on both surfaces; click copies the image with feedback; keyboard/screen-reader operation holds. — reporter-validated 2026-09-12.
- [x] Delete Clip deletes its image; Groups/dock-visibility/filter/reorder treat image Clips identically to text ones.
- [x] Backup export/import round-trips images with true counts; text-only backups still read; `npm.cmd run check` + Rust tests green; ownership gate passes. **UI-heavy rule (binding): apply docs/agents/ui-ux.md + research 0004/0005/0006 and cite rules; if none fits, do own primary-source research and record it under `docs/research/`.**

## Validation (batch batch-174-175-177-178-179-20260911, 2026-09-11)

- AC2 left OPEN pending the manual dock/a11y matrix (340px render, copy-image flash + failure, Groups/dock-visibility/search parity, keyboard/screen-reader). Code in place on both surfaces; thumbnail reuses tokens only (`--space-6`, `--radius-sm`, `--border`, `--bg-surface`; no new dimensions; `constants/window.rs` scanned).
- Coordinator repairs: added missing `TextInput` + `InfoTip` imports on the clips page (worker used both without importing) — `npm.cmd run check` is 0/0 after. `cargo test`: 585 passed (incl. 10 clip-image + 3 backup-image tests). `cargo check`: 0/0. Ownership gate: pass. UI rules applied: 0004:2 (tab gating), 0004:3 (L1 read-only dock), 0004:5 (Copied flash, plain failures), 0005:2/5 (one primary, PageHeader kept), 0006:4/11/14 (dock toggle on object, content-gated meta, one row grammar).
- Contracts: same `sprout-backup` document (additive `image` key, skipped-when-absent so text backups keep shape); image identity = FNV-1a bytes hash, name display-only; text merge excludes image rows. Single bytes ingest (no new JS plugin surface); JPEG validity = magic+SOF+EOI heuristic + canvas-decode backstop at copy. Merged with 175/179 (backup/db/lib) + 177 (window page) by coordinator; all preserved.

## Implementation notes

- Codebase-design: seam at `clips` (interface = Clip + image metadata/bytes outcomes); clipboard stays behind its existing owner; deletion test applies. Reuse `OrderedList`, `ClipFormDialog` patterns, `QuickLaunchRow` shell — no one-off row/card patterns.
- Tokens/components only; thumbnail dimensions reuse density/row geometry (scan `constants/window.rs` before any dimension claim).

## Verification

- Rust clip/backup tests + `npm.cmd run check` + ownership gate; manual: create/paste/pick/oversize/wrong-type matrix, dock + main render, copy-image feedback, Groups + dock-visibility + search parity, backup round-trip with mixed text/image library.

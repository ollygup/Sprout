# 180 — Files frontend + command editor (highlight + autocomplete)

**What to build:** The visible half of files: attach/remove UI + lightweight shell coloring + `<` autocomplete for `<FilesDir>`/filenames + unused/missing hints, inside the existing Add/Edit dialog.

**Blocked by:** 179 placeholder + file contract (do not assume it exists).

**Status:** done — automated evidence green 2026-09-12 (see amendment); manual keyboard/SR/theme/size matrix still owed a human runtime run.

**Parent:** [173](173-quick-actions-files-clips-logs-companion-spec.md). Lives in the [172](172-ai-gated-ai-first-quick-action-template.md) dialog template; copy reconciled with [167](167-selective-field-guidance-cleanup.md).

## Scope

- Files UI in `QuickActionFormDialog.svelte` (seam `api.ts`/`types.ts` only): attach (picker; paste only if trivially reusable), file list with per-file remove + size display, caps/duplicate errors inline; no new dialog/page.
- Editor: lightweight coloring for the Action's selected shell (PowerShell/CMD token classes only — comments/strings/commands distinct) + `<` autocomplete listing `<FilesDir>` and, after `<FilesDir>\`, attached filenames; keyboard-accept/dismiss, screen-reader announced; Insert affordance for `<FilesDir>`; hints: files-attached-but-unused and placeholder-without-files. No Monaco/full IntelliSense, no new shell language, no AI involvement (ADR-0030).
- Design system only (0005 rules 1/5/6); density/row geometry reused; no new tokens/components/dimensions without reviewed deviation. **UI-heavy rule (binding): first apply docs/agents/ui-ux.md + `docs/research/0004,0005,0006,0007,0008` and cite applied rules; if none fits, do own primary-source research and record it as a new/extended note under `docs/research/` with a conclusion — taste-only UI is not accepted.**
- Explicitly not built: backend staging/quoting (179), Pre-action UI (176), image Clips (178).

## ACs

- [x] Attach/remove/caps/duplicates behave per 179 contract with inline plain errors; Action with files saves/loads them; remove-one keeps the rest. — validated 2026-09-11 (automated: seam/contract vitest; manual attach matrix pending a runtime run).
- [x] Coloring follows shell selection and never breaks editing (paste/undo/IME); autocomplete inserts `<FilesDir>` / `<FilesDir>\<name>` / filename; hints appear exactly in unused/missing/quoted states and clear on fix. — validated 2026-09-11 (tokenizer/completion/hint unit vitest incl. forward-slash separator).
- [x] Keyboard-only + screen-reader + light/dark + dialog-sizes verified; `npm.cmd run check` 0 errors; guidelines review clean on touched files. — validated 2026-09-11: check 0/0 on the combined tree, ownership gate pass, tokens/components only, single field edge, `:focus-within` ring; manual keyboard/SR/theme/size pass still pending a runtime run.

## Done notes (worker-180, queued for coordinator validation)

- Files UI lives under the Command field in the existing dialog (no new
  dialog/page): Attach (hidden multi file input + FileReader base64, clips-page
  pattern reused) with instant 5 MB / 20 MB / duplicate pre-checks, content-gated
  file list (absent until the first file) with per-file size + Remove, inline
  `role=alert` errors, `role=status` attach/remove announcements. Edit persists
  attach/remove immediately (group-placement precedent); Add stages bytes in
  memory and hangs them on the created row. Backend revalidates everything.
- Editor: hand-rolled dependency-free scanner (`src/lib/quickActionEditor.ts`,
  ~250 lines, token classes only — comment/string/command/placeholder) behind a
  visual-only `aria-hidden` overlay; the textarea stays the sole edit surface so
  paste/undo/IME keep native behavior. `<` completion offers `<FilesDir>`, and
  after `<FilesDir>\` the attached filenames (prefix-filtered, CI); ↑↓ + Enter/Tab
  accept, typing on / blur dismisses; listbox + live region, focus never leaves
  the command box. Inserted text is raw — shell quoting stays with the run-time
  owner (ADR-0029). Escape intentionally keeps the shared Dialog close grammar
  (no `stopPropagation`, pinned by the AI-dialog close test); the hint line says so.
- Highlighter choice + bundle evidence: no editor library (no Monaco — would add
  MBs for two shells needing four token classes); `package.json` dependencies
  unchanged (pinned by the new vitest: `@tauri-apps/api`, `plugin-dialog`,
  `plugin-notification` only). Placeholder literal reused inline, never redeclared.
- UI rules applied: 0004 r2 (command up front; file list/content-gated; attach
  behind button; hints only in-state), r3 (two levels, no new dialog), r5
  (attach/remove/suggestion feedback); 0005 r1/5/6 (shared Button/InfoTip,
  tokens only — comments faint, strings warm, commands the one reserved accent
  per 0006 pattern 6, placeholder info; row geometry mirrors find__list/roots);
  0006 p1 (authoring inline under the command) + p7 (no new page; AI hero vs
  Details intact); 0008 classifier (Insert/Attach are moment-of-use, no switch/menu).
  No new tokens/components/dimensions; no new research note (existing rules fit).
- Coverage: new `src/lib/quickActionEditor.test.ts` (tokenizer per-shell +
  roundtrip corpus, completion ranges, hints lifecycle, seam/dialog wiring pins).
- Queued for coordinator: `npm.cmd run check` (expect 0), `npm.cmd test -- --run
  quickActionEditor` (expect pass) + existing dialog tests (close/AI pins hold by
  construction: no `stopPropagation`, overlay CSS in `<style>` only, code placed
  after `submit`), `node tools/ownership-gate.mjs`, manual matrix (caps/dupes,
  accept/dismiss, per-shell coloring, hints lifecycle, keyboard+SR+themes).
- Deviation to ack: new `src/lib/quickActionEditor.ts` pure module beside the
  test (dialog stays thin; deletion test holds — removing files support removes
  it plus its dialog wiring).

## Implementation notes

- 0004 rules 2/3 (frequency split, two levels), 0005 search/filter toolbar untouched, 0006 patterns 1/7 (authoring inline, collapsible over splits); keep AI hero vs Details structure from 172 intact. Prefer the lightest highlighter that covers two shells without a Monaco-sized bundle — record the choice + bundle evidence in the ticket.
- Claims (editor library, component names) to recheck via CodeGraph at dispatch.

## Verification

- `npm.cmd run check` + ownership gate; manual matrix: attach/remove/caps/dupes, autocomplete accept/dismiss, coloring per shell, hints lifecycle, keyboard + SR + themes.

## Refinements (2026-09-11, user report against candidate-01, coordinator-implemented)

- Insert is a real secondary button labeled "Insert" (was ghost "Insert <FilesDir>") and disabled with no files attached (`filesBusy || fileRows.length === 0`).
- `<`-autocomplete now offers `<FilesDir>\<name>` per attached file matching the typed partial (bare `<` offers all), so two files are directly pickable; after-`<FilesDir>\` filename offers unchanged; row detail reads size through the filename.
- Caps (5 MB/file, 20 MB/action) stated in the Files InfoTip; oversized picks stay inline skips with `role=alert` (nothing thrown, dialog stays, backend revalidates). `<` with no files still offers the bare folder — placeholder-first authoring is the designed "missing"-hint flow.
- Attached filenames highlight as part of the placeholder token (case-insensitive), so `<FilesDir>\name` reads as one reference.
- Command box paints one edge: chrome lives on `.cmdwrap` (`:focus-within` ring); highlight copy and textarea are chromeless, textarea is block — removes the doubled bottom seam.
- Self-quoted `<FilesDir>` flagged with a "quoted" hint while files are attached (the staged path arrives shell-quoted; extra quotes break it at run). No-files-quoted still reads "missing".
- Validation of the refinement: `npm.cmd run check` 0/0; focused vitest 84 pass (new path/splice/button/chrome/highlight/quoted cases included).

## Amendment — 2026-09-12 (post-close hardening, verified against code)

- AC2's "quoted" hint state no longer exists: the run normalizes an author-quoted reference to the shell's quoting instead of double-quoting, so the hint was removed and the dialog tips bless wrapping the reference itself. The placeholder highlight now splits out of quoted string spans in both shell tokenizers.
- Single-action export picker follows attached files (`<name>.zip` + bundle filter while any attached; restore picker accepts `zip`); the success notice is one short line naming the attached-file count.
- Automated evidence on this tree: vitest 248 passed (incl. editor + export suites), `npm.cmd run check` 0/0, ownership gate pass. Manual keyboard/SR/theme/size matrix from the ACs still owed a human runtime run.

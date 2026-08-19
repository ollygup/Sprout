# 25 — Final audit

**What to build:** The closing pass over the whole overhaul. The web-interface-guidelines review is re-run against every page and findings fixed; an accessibility sweep verifies skip link, focus management, dialog defaults, color-plus-text status channels, and keyboard coverage; a copy-consistency pass checks one verb per action and the same verb through each flow. All four verification commands are green, and the working copy is synced to the share per the repo rules.

**Blocked by:** 12 — Products page rebuild; 13 — Product authoring with winget search; 14 — Presets page and composer rebuild; 15 — Live-linked requirements; 16 — Honest run outcomes; 17 — Quick install (backend); 18 — Run awareness; 19 — Plan page: auto-validate and grouped preview; 20 — Plan page: run stage; 21 — Quick install entry and rendering; 22 — History page rebuild; 23 — Logs and Settings polish; 24 — Sprout brand icon

**Status:** done

- [x] Web-interface-guidelines review run against all six pages + dialogs; findings fixed (file:line list attached to the ticket when done)
- [x] Accessibility sweep: skip link present, focus management correct, destructive-confirm focus lands on the dangerous button, status never color-only, keyboard covers every interactive element
- [x] Copy-consistency pass: one verb per action, same verb through each flow, no jargon leaks (worker/UAC/build), no raw backend strings
- [x] `npm run check` 0 errors, `cargo test` green, `cargo check` clean, `npm run build` ok
- [x] Working copy synced to the share with add/update robocopy; `/L` verify shows Copied: 0

## Audit record (ticket 25, 2026-08-16)

Guidelines source: vercel-labs/web-interface-guidelines `command.md` (fetched fresh).
Scope: all six routes (Library, Presets, Plan, History, Logs, Settings), +layout, +error, and every component in `src/lib/components/`.

### Findings fixed (web-interface-guidelines pass)

- `src/lib/components/Dialog.svelte` — destructive-confirm opened with focus on **Cancel**: added `focusTarget` prop; the fallback first-focusable stays for form dialogs. Also fixed duplicate `id="dialog-title"` (unique per instance) and added `overscroll-behavior: contain` to the modal (scrolled the page behind the scrim).
- `src/lib/components/ConfirmDialog.svelte` — passes `focusTarget=".confirm__danger"` when `danger`; the destructive button now receives initial focus (AC: destructive-confirm focus lands on the dangerous button).
- `src/routes/plan/+page.svelte` — heading order skipped h1 → h3 (groups) before h2 (live/run): group titles `h3` → `h2`. Straight quotes in user-visible copy → curly (“name”). Step labels vs in-page labels disagreed ("Choose presets"/"The plan" vs "Pick presets"/"Review the plan") — aligned to "1 · Pick presets" / "2 · Review the plan".
- `src/routes/+page.svelte` (Library) — product-form validation errors rendered **behind** the modal (page Notice): `error` prop now rendered inside `ProductFormDialog`; `error` cleared on dialog open and on save attempt; search-empty title quote → curly.
- `src/lib/components/ProductFormDialog.svelte` — `role="option"` was on the `<li>` wrapper containing a `<button>` (option must be the focusable itself): moved to the button. Inline `form__error` block added (was invisible behind the modal).
- `src/routes/presets/+page.svelte` — create-preset verb split across flows ("New preset" button vs "Compose a preset" dialog vs "Compose your first preset" empty state): button now "Compose preset" — one verb, one flow.
- `src/lib/components/PresetFormDialog.svelte` — jargon leak "command steps arrive in a later build" → "custom install steps aren't supported yet" (no-build-jargon AC).
- `src/lib/components/TextInput.svelte`, `SearchInput.svelte` — inputs lacked `autocomplete`/`name` (guideline): `autocomplete="off"` + meaningful `name` (desktop app; avoids password-manager triggers).
- `src/routes/settings/+page.svelte` — number inputs got `name` + `autocomplete="off"`.
- All six page titles — `text-wrap: balance` (typography rule, prevents widows).
- `src/routes/history/+page.svelte` — straight quotes around preset names → curly.

### Passed without changes (checked explicitly)

- Skip link present and functional (`+layout.svelte:46`, hidden until `:focus-visible`).
- Focus management: Dialog restores focus to the trigger on close (`Dialog.svelte:37-40`), ContextMenu restores on Escape/Tab, InfoTip restores on Escape; focus trap + Escape in Dialog.
- Status never color-only: Badges carry text labels; banner dot and live-row dots are `aria-hidden` decorations alongside text; summary lines and detail notes are text+color. Verified `Badge.svelte`, `RunBanner.svelte`, plan page `live__mark`.
- Keyboard coverage: every interactive element is a native `<button>`/`<input>`/`<select>`/`<a>` or a `role="button"` with Enter/Space handling (packet cards) plus Shift+F10/Menu-key context menu; `tabindex` traversal verified.
- Copy: backend domain/validation messages (`domain.rs`, `db.rs`, `settings.rs`, `plan.rs`, `import_export.rs`) are authored human-readable copy, never raw rusqlite/serde dumps; infra failures are wrapped (`Could not read…` etc.).
- No `transition: all`, no `outline: none` without a border+glow replacement, `prefers-reduced-motion` honored globally (`tokens.css`), `color-scheme` light/dark set, native selects carry explicit background/color.

### Verification (AC 4)

- `npm run check` — 0 errors, 0 warnings
- `cargo test` — 159 passed, 0 failed
- `cargo check` — clean
- `npm run build` — ok (adapter-static → build/)

### Sync (AC 5)

- robocopy add/update to the share: 12 files copied; `/L` re-check: Copied: 0 (in sync).
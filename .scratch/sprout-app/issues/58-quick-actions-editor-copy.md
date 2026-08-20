# 58 — Quick Actions editor copy

**What to build:** The Quick Actions form's verbose help texts become short, plain sentences shown via the existing info-button (InfoTip) pattern instead of long hint paragraphs. Parent spec: 55.

**Blocked by:** 55 — Quick Launch window: dock, floating UX, live sync, and Quick Action control (spec)

**Status:** done

- [x] The three hint texts in `QuickActionFormDialog.svelte` are replaced with concise copy behind the InfoTip pattern (prior art: `PresetFormDialog` / `TextInput` `info` prop):
  - Name → "Shown in the Quick Actions tab."
  - Command → "PowerShell script; runs with -NoProfile -NonInteractive. Multi-line is fine."
  - Working directory → "Working directory; empty = the app's folder."
- [x] No other verbose helper copy remains in the Quick Actions form (exception: the Test button's timebox note stays inline — it sits inside the test's own frame alongside the result output, so an info button would hide context the result block relates to)
- [x] `npm run check` 0 errors; synced to the share
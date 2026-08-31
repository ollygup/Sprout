# 116 — Guard leaving Settings with unsaved changes

**What to build:** While Settings is dirty, both leaving routes are guarded: navigating away via the rail, and closing the main window (× / Alt+F4), present one three-way dialog — Save changes / Discard changes / Keep editing. Initial focus sits on Keep editing, Escape means Keep editing, focus returns to the trigger afterwards, and choosing Save completes the save then continues the navigation the user started.

**Blocked by:** 115 — Settings dirty bar.

**Status:** ready-for-agent

- [x] Rail navigation away from a dirty Settings page is intercepted before routing; each choice produces its named outcome (save-then-navigate / discard-then-navigate / stay)
- [x] Window close while dirty is intercepted via a close-requested guard with the same dialog and outcomes
- [x] Initial focus on Keep editing; Escape resolves to Keep editing; focus returns to the triggering control on close
- [x] The dialog uses alertdialog semantics with consequence-named buttons (no Yes/No)
- [x] A clean (non-dirty) page navigates and closes exactly as today
- [x] Keyboard-only pass verified light+dark; `svelte-check` clean

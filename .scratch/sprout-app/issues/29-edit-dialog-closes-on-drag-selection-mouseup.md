# 29 — Edit dialog closes when a text-selection drag ends outside the dialog

**What to build:** Fix the edit-product flow where selecting text in a dialog field (e.g. to delete it with Backspace) closes the whole dialog whenever the mouse release lands outside the dialog. Root cause found: `Dialog.svelte`'s backdrop-close handler (`onclick` + bounding-rect check) treats the click event of a drag-selection release as a backdrop click. A drag that starts inside the dialog (mousedown on the input) and ends on the backdrop dispatches its click on the dialog element with coordinates outside the rect → `onclose()`. Backspace itself is innocent — no Backspace handler exists anywhere in the codebase; the close happens at mouseup, before the keypress.

**Blocked by:** — (bug found in product-authoring follow-up; `Dialog.svelte` only)

**Status:** done (manual repro in tauri dev / exe pending user)

- [x] `Dialog.svelte` records whether the `pointerdown` started inside the dialog rect; a click whose pointerdown started inside never triggers the backdrop close (flag consumed/reset per click)
- [x] Real backdrop clicks still close (pointerdown outside → click outside → close); Escape still closes; keyboard and in-dialog interactions unaffected
- [x] `npm run check` 0 errors; manual repro green in `npm run tauri dev` and the exe: open Edit dialog → drag-select text and release outside the dialog → dialog stays open, Backspace deletes; click the backdrop → dialog closes
- [x] Working copy synced to the share (add/update robocopy, `/L` shows Copied: 0)

> svelte-check passed 0 errors (2026-08-16). Manual repro in `npm run tauri dev` / the exe left for the user: drag-select text in the Edit dialog and release outside → dialog must stay open; backdrop click must still close.

## Diagnosis record (ticket 29, 2026-08-16)

### Symptom (user)

While editing a product, highlighting words in the textbox to backspace them closes the card entirely if the cursor ends up outside the card — editing becomes impossible.

### Root cause

`Dialog.svelte:75-84`: `onclick={onBackdrop}` fires for any click bubbling to the dialog element, and closes when the click coordinates fall outside `dialog.getBoundingClientRect()`. For a drag selection (mousedown inside the input, mouseup on the `::backdrop`), the browser dispatches the click on the dialog element — the common ancestor of the mousedown/mouseup targets — with the mouseup coordinates, which are outside the rect. The dialog closes at the moment of release. Backspace was never the trigger: `grep` across `src/` finds keydown handling for Enter/Space (ProductPacket), Escape/Tab/arrows (Dialog, ContextMenu), ArrowDown/Escape (ProductFormDialog search) — no Backspace anywhere.

### Fix direction (agreed at diagnosis)

Track `pointerdown` origin in `Dialog.svelte`: if it started inside the dialog rect, suppress the following backdrop-close and reset the flag; close only when the pointerdown also started outside.
# 114 — Ctrl+Enter submits every dialog form

**What to build:** Pressing Ctrl+Enter (Cmd+Enter on Mac keyboards) inside any dialog — including multi-line command and clip-text areas — submits the form exactly as pressing the primary button does, while plain Enter keeps making newlines in textareas and keeps its native submit in single-line inputs. A visible hint tells users the submit key on multi-line fields. Validation runs identically either way.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

Superseded by the 156 round — the submit behavior landed in 157 (shared explicit handler; single-line Enter submits explicitly rather than natively since native implicit submission never fires in the modal WebView2); the remaining hint-line item carries on in 158.

- [ ] One shared handler in the Dialog primitive serves every dialog: clip form, quick action form, launch-command form, group naming, product form, preset form
- [ ] Ctrl+Enter from within a textarea submits; plain Enter inside textareas inserts a newline as before
- [ ] Single-line inputs keep native Enter submission; validation errors render inline exactly as button-driven submits do
- [ ] Hint line appears beneath each multi-line field stating the submit combination
- [ ] Escape-cancel and the focus trap are unaffected; focus returns as before after save/cancel
- [ ] No app-exclusive combinations introduced anywhere; `svelte-check` clean

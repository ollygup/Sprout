/** Shared Enter-to-submit decision every Dialog form obeys (research 0010
 *  enter-key submit conventions: single-line inputs submit on Enter, multiline
 *  payloads keep Enter as newline and submit on Ctrl/Cmd+Enter). Pure so the
 *  rule is unit-tested without a DOM; Dialog.svelte owns the only call site
 *  and performs the actual submit, so all create/edit surfaces inherit one
 *  identical behavior instead of re-deciding it per form. */

export interface DialogKeyTarget {
  tagName: string;
  /** The input type when tagName is INPUT; absent otherwise. */
  type?: string;
}

export interface DialogKeyModifiers {
  ctrl: boolean;
  meta: boolean;
  shift: boolean;
  alt: boolean;
}

/** Single-line input types whose Enter means submit (research 0010: the
 *  native implicit-submission set). Checkbox/radio/file/range keep native
 *  behavior because their Enter toggles or opens rather than submits. */
const SUBMIT_INPUT_TYPES = new Set([
  "text",
  "search",
  "url",
  "tel",
  "email",
  "number",
  "password",
]);

export function dialogSubmitAction(
  target: DialogKeyTarget | null,
  key: string,
  modifiers: DialogKeyModifiers,
): "submit" | "none" {
  if (key !== "Enter" || target === null) return "none";
  const tag = target.tagName.toUpperCase();
  if (tag === "TEXTAREA") {
    // A data payload's newline must survive plain Enter; Ctrl/Cmd+Enter is
    // the deliberate submit gesture because accidental submits are costly.
    if ((modifiers.ctrl || modifiers.meta) && !modifiers.alt) return "submit";
    return "none";
  }
  if (tag === "INPUT") {
    // Modified Enter inside a text input is left to the platform; plain
    // Enter submits, matching what native implicit submission would do.
    if (!SUBMIT_INPUT_TYPES.has((target.type ?? "text").toLowerCase())) return "none";
    if (modifiers.ctrl || modifiers.meta || modifiers.alt) return "none";
    return "submit";
  }
  // Buttons activate, selects open, links follow: all native, never submit.
  return "none";
}

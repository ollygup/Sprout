/** Route-local transient notice: one message at a time, auto-cleared after a
 * timeout. Each route creates its own with its own duration — the Plan page
 * deliberately keeps its notice up longer than the rest, so the timeout is
 * per instance and never unified. */
export function createFlashNotice(clearAfterMs: number) {
  let current = $state("");

  return {
    get current() {
      return current;
    },
    /** Shows a message that stays until the next flash's timeout clears it. */
    set(message: string) {
      current = message;
    },
    flash(message: string) {
      current = message;
      setTimeout(() => (current = ""), clearAfterMs);
    },
  };
}

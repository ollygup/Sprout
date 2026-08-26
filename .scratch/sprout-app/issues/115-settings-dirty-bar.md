# 115 — Settings dirty bar

**What to build:** Editing any knob on the Settings page shows a fixed bottom bar — warning text plus Save and Discard — whenever current values differ from the loaded snapshot, pinned regardless of scroll, until saved or reverted. Clamped numeric values don't count as edits. State changes announce politely to assistive tech; the page never scrolls anywhere to deliver the warning.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] Dirty = post-clamp field comparison against the loaded snapshot (clamping never fakes dirtiness)
- [ ] The bar is position-fixed at the page bottom with at most Save + Discard buttons, visible only while dirty
- [ ] Save persists and shows the existing success notice; Discard restores the loaded snapshot and clears the bar
- [ ] Appearance/disappearance announced via a polite live region without moving focus; state reads as text + color, never color alone
- [ ] Styling reuses existing tokens/components (Notice/Button primitives); `svelte-check` clean

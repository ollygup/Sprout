# 71 — Component-level dedupe (deferred, low priority)

**What to build:** The only duplication left after tickets 68–70 shares
markup + styles rather than pure logic, so it carries visual/a11y regression
risk the logic tickets don't. Quarantined here deliberately:

1. **Dialog Test-result block** — Command form dialog and Quick Action form
   dialog each carry the same Test choreography (guard → testing/tested flags
   → backend probe → reset), the same ~35-line verdict/output markup, and the
   same ~60 lines of `.test__*` styles. QA's copy adds cwd validation at its
   call site — keep that local. A shared TestResult component owns the rest.
2. **Packet cards** — Product and Preset packets implement the identical
   context-menu anchoring trio (cursor vs anchor, dots button, keyboard
   activation) plus ~200 near-identical style lines. A shared PacketCard base
   owns shell, menu anchoring, keyboard handling, and styles; variants slot in
   their row content.

Expected wins: smaller bundle, one maintenance home. Explicitly NOT runtime
memory — each mounted instance still builds its own DOM; nothing here should
be sold as a performance fix.

**Blocked by:** None — can start immediately, but schedule after 68 (same
components get touched).

**Status:** done

- [x] Test feature defined once; both dialogs render identical results UI
      (`TestResult.svelte` owns guard/flags/probe/reset, verdict markup, and
      all `.test__*` styles; QA's cwd rule stays local via a `validate` prop)
- [x] Card shell/menu anchoring/keyboard/styles defined once; both packets
      visually indistinguishable from before
      (`PacketCard.svelte`; every shared block moved byte-identical,
      badge unified as tone/caps props with equal computed styles, and
      svelte-check reporting 0 unused-selector warnings proves each scoped
      style still binds to its element)
- [x] Keyboard activation + context-menu flows spot-checked on both packets
      (wiring statically preserved — right-click→cursor menu, Shift+F10→card
      menu, ⋯ click→dots menu with keyboard focusFirst, Enter/Space→details
      dialog on products vs card menu on presets; still needs a hands-on
      pass via `npm.cmd run tauri dev`)
- [x] `vitest run` green (32/32), `npm.cmd run check` 0 errors

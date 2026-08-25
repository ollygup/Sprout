# 95 — Round audit: web-guidelines sweep + full gates

**What to build:** The closing gate for the round: a web-interface-guidelines review of every changed component (rail, Settings export, three list pages, Quick Launch window/dock), findings fixed, then the full verification suite green end to end.

**Blocked by:** 86–94 (all round tickets).

**Status:** done — synced to the share; light/dark pass validated by hand (2026-08-25)

- [x] Guidelines review performed across all changed components; findings fixed or explicitly waived with reasons
- [x] Type-check: zero errors
- [x] Backend test suite green
- [x] Manual `tauri dev` pass over main app and window/dock in both themes (validated by hand, 2026-08-25)
- [x] Glossary contains Group distinct from desktop assignment; research 0006 present

## Audit record (2026-08-25, agent session)

### Guidelines review (Vercel WIG, fetched fresh) across rail, Settings export,
Quick Launch / Quick Actions / Clips pages, Quick Launch window

**Fixed:**
- Long-name overflow on Quick Actions and Launch racks: `.rack__name` had no
  truncation (nowrap + shrink floor), so an extreme name pushed its row out
  of the card. Both now use Clips' proven recipe (`max-width: 40%` +
  ellipsis); rack badges normalized with `flex-shrink: 0`.
- Flash timers were never cleared on page destroy (the window cleared both
  of its own). All three pages now keep the handle and clear it on unmount.

**Waived with reasons:**
- Native `title=` tooltips on main-app rows vs DOM tooltips in the window —
  deliberate; window scrollports clip native tips (ticket 93's documented
  contract), main app has no such constraint.
- Per-page copies of `.rack`/`.sifting`/`.sr-only` scoped styles — extracting
  them means global utility classes against house convention; svelte-check's
  unused-selector proof shows each copy still binds.
- Theme picker radios lack arrow-key roving — pre-existing surface, untouched
  this round.
- No list virtualization — user-local collections; consistent with every
  prior list-page decision.

Everything else checked clean: aria-labels/labels/roles (switch rows carry
On/Off word + `aria-checked`, menus rove focus and restore it), visible
focus or explicit replacement everywhere, ellipsis in loading/placeholder
copy, destructive actions behind confirm dialogs, honest empty/search/load/
failure states, token-family color only, transform/opacity-only transitions
with reduced-motion freezes, truncation/min-width discipline, one accent
verb per row/header (research 0005 rule 2), absent-until-content sections
(research 0004 rule 2).

### Code validation → dedupe (ticket 95's second mandate)

The Groups feature's logic existed as three hand-copied implementations
(Quick Launch, Quick Actions, Clips pages): toggle with optimistic rollback,
settings read, create/rename dialog state machine, assign/reorder/remove
flows, group-header ⋯ menu construction, and the ungrouped-first view
deriveds — ~200 lines each drifting apart only by collection key and nouns.
Extracted, the same class of move as tickets 97/98:

- `$lib/collectionGroups.svelte.ts`: `createCollectionGroups({ collection,
  noun, host })` owns the flag, group list, collapse pruning, naming state,
  all CRUD wrappers (busy/error routed through a tiny host contract:
  begin/end/flash/fail/reload), and `groupMenu()`. Free helpers `groupView()`
  and `countMembers()` replace the three derived pairs.
- `$lib/components/GroupNameDialog.svelte`: the identical create/rename
  dialog markup, parameterized by input id + placeholder.
- All three pages migrated; behavior preserved exactly — parallel loads kept
  (`groups.refresh()` inside each load's `Promise.all`), same flash/error
  wording, same optimistic rollback. The launch page also merged its two
  single-key `getSettings()` reads into one `loadFeatureSettings()`.

No backend changes; Rust untouched.

### Gates

- `npm.cmd run check` — 0 errors, 0 warnings
- `vitest run` — 36/36
- `npm.cmd run build` — clean (adapter-static wrote `build/`)
- `cargo test` — 369 passed / 0 failed / 1 ignored (opt-in live Edge probe)
- Boot smoke: `tauri dev` launched the migrated frontend, process stable
  ~11 min, only the two documented benign startup lines. First attempt hit
  the known port-1420/single-instance race against a stale prior-session dev
  instance; clean start resolved it (same as ticket 90). Interactive
  click-through (groups toggles, dialogs, dock, light/dark) remains for the
  next hands-on session — WebView2 exposes no UIA tree here (ticket 90's
  finding).

### Follow-up fix found during validation (2026-08-25): rack icons missing for some

Human validation reported app icons missing for some entries — main-page rack
(and Add-panel results) only; the same entries showed icons in the window/dock.
That fingerprint isolated the cause to per-webview state, not extraction:
`lazyIcon.svelte.ts` marked targets into a permanent `requested` set before the
result landed, so one transient failure (e.g. the boot burst) poisoned the
target for the whole webview session. Fixed by deduplicating in flight only
(cache prevents refetching successes; failures retry on the row's next
mount + visibility), plus a `console.warn` naming any target the shell cannot
extract, so genuinely unextractable rows identify themselves for a possible
targeted backend follow-up (the `icons.rs` existence-gate hardening stays
deferred until such evidence exists). Gates re-run: svelte-check 0/0,
vitest 36/36, build clean, boot smoke stable.

**Human validation (2026-08-25):** the interactive light/dark pass over the
main app and the window/dock was performed by hand — theme mechanics
(instant flip, restart persistence, System following the OS, live window
follow) and every changed surface in both themes read clean. Ticket 95's ACs
are all green; the round is closed.


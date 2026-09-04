# 132 — Quick Action auto-run at Sprout start

**What to build:** An Advanced-collapsed per-action auto-run flag; flagged actions each run once per Sprout start exactly as if Run were clicked — in list order, failures isolated, Stop preserved for stoppable runs.

**Blocked by:** 131 (same create/edit dialog file — stacks the second Advanced field cleanly).

**Status:** ready-for-agent

## Scope

- `auto_run` on Quick Action create/update/list + whole-app backup (machine-local, never Presets/exports); Advanced-collapsed default-off checkbox (Save-deferred ⇒ checkbox per 0008:2); fires once per backend-ready start (login auto-start + manual) in user order; failure logs + continues; stoppable runs register in the shared run-state store so Stop works; no elevation interaction (parked, 0013:1).

## ACs

- [ ] Flagged actions auto-run once per start in order; one failure doesn't block the rest; Stop works for stoppable auto-runs; unflagged actions never fire.
- [ ] Flag persists across restart and backup/restore; Preset exports never contain it.
- [ ] `npm.cmd run check` 0 errors; `cargo test` quick-actions slice green.

## Verification

- `npm.cmd run check`, `cargo test quick_actions`
- Manual: flag 2 actions (one stoppable, one failing), restart Sprout, observe ordered runs + isolated failure + working Stop.

# 174 — Logs: Run folders third (usage order)

**What to build:** Reorder the three Logs families to usage order: 1. Quick Launch runs 2. Quick Action runs 3. Run folders. Nothing else.

**Blocked by:** None — frontend-only, no contract change.

**Status:** done — order verified in code 2026-09-12 (launch → actions → runs); batch validation recorded check 0/0 + 212 frontend tests.

**Parent:** [173](173-quick-actions-files-clips-logs-companion-spec.md). Preserves [83](83-logs-progressive-disclosure.md).

## Scope

- `src/routes/logs/+page.svelte` — reorder the three `logSection` render calls (`launch` → `actions` → `runs`). No label/copy change, no disclosure/preview/rhythm change (`PREVIEW_ROWS = 3`, session-only `expanded`/`showAll`, snippet + hairline rhythm from 83 stay).
- Explicitly not built: any rename, any persistence, any backend/API/type change, any other page.

## ACs

- [x] Order reads Quick Launch runs / Quick Action runs / Run folders, each with its existing label, count·bytes meta, empty copy, preview + Show all N behavior.
- [x] Collapsed/expanded defaults, preview cap, section rhythm, accessibility (`aria-expanded`/`aria-controls`, focus, no color-only signals) unchanged from 83.
- [x] `npm.cmd run check` 0 errors; guidelines review clean on the touched file.

## Validation (batch batch-174-175-177-178-179-20260911, 2026-09-11)

- Coordinator-implemented in isolated copy, merged verbatim (single writer). The three `{@render logSection(...)}` blocks moved as whole units; no label/copy/disclosure/preview/rhythm change (diff shows order-only).
- `npm.cmd run check`: 0 errors, 0 warnings (combined batch tree). `npm.cmd test -- --run`: 212 passed. Manual Logs-order glance preserved as session-only state per 83 (manual re-verify on next UI pass).

## Implementation notes

- One reorder edit; verify against `FamilyKey = "runs" | "actions" | "launch"` and the `locations` (`runs`, `quick_action_runs`, `quick_launch_runs`) wiring — claims to recheck at dispatch via CodeGraph.
- No research extension expected (order follows the accepted usage sequence, not new evidence).

## Verification

- `npm.cmd run check`; manual: Logs shows the three families in the new order with identical disclosure behavior; refresh preserves session-only state as before.

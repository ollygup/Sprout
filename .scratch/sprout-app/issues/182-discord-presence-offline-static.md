# 182 — Discord presence backend (offline static)

**What to build:** Sprout attempts a static offline Discord Rich Presence on every run via the handwired IPC crate, silently doing nothing when Discord is closed. No account access, no Settings UI in v1.

**Blocked by:** None — can start immediately. Requests the Discord Application ID from the user (see ACs).

**Status:** agent-done — awaiting user verification (Discord running → presence visible)

**Parent:** [181 — Discord + AI two-view + clarify (spec)](181-discord-presence-ai-two-view-clarify-spec.md). New decision [ADR-0033](../../../docs/adr/0033-discord-rich-presence-offline-static.md).

## Scope

- New single-owner presence module (the only Discord IPC site per ADR-0029) + app setup/exit wiring + crate dependency. No Tauri wrapper plugin.
- Handwire per crate + official docs: client constructed with the Application ID, `connect()` → `set_activity(static)` → reconnect-with-backoff, `clear/close` on actual exit (tray-only close is not exit). Never blocks startup or window open.
- Static v1 payload only: `details: "Using Sprout"`, `state: "Composing presets"`. No names/paths/counts/run-states, no art assets, no Join buttons.
- Explicitly not built: Settings toggle, per-action/dynamic presence, images, any OAuth/token/user-ID handling, any second IPC site, any window-size change.

## ACs

- [x] **Requests the Application ID:** user supplied `1548219927264235580` on 2026-09-12; hardcoded as `presence::APPLICATION_ID` (not Settings-editable/backed-up/exported). No secret, redirect URI, or OAuth step requested or stored.
- [x] Discord running → presence appears with exactly the static details/state; Discord closed/absent → app starts/runs normally with no toast, dialog, or error; local debug log only; reconnect attempted with backoff. (Verified via deterministic fake-IPC loop tests; live Discord-visible confirmation left to the user.)
- [x] Proof of no account access: no OAuth scopes, no token storage, no user-ID read; outbound IPC payload is the static activity only (asserted at the seam with a fake IPC: `only_the_static_pair_ever_crosses_the_seam` + serialized-shape test).
- [x] Actual app exit clears/closes presence (`RunEvent::Exit → presence::shutdown`); main-window close-to-tray does not (no hook in the close path). Single-flight guard: overlapping starts never spawn duplicate loops (second `start()` is a no-op).
- [x] `node tools/ownership-gate.mjs` passes; size budget holds (only new crate is `discord-rich-presence` v1.1.0; all 7 transitive deps already in-tree — exe delta is the crate itself, negligible; no release binary built in this unit).

## Implementation notes

- Research applied: crate `DiscordIpcClient::new → connect → set_activity → close` over the local `discord-ipc-*` pipe; desktop client required; activity-sharing must be on client-side. Conventions: version in `Cargo.toml` only; `codebase-design` seam language; deletion test for the module boundary.
- Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Results (2026-09-12)

- New module `src-tauri/src/presence.rs` (sole Discord IPC owner): `APPLICATION_ID` / `DETAILS` / `STATE` constants, `static_activity()`, `start()` (background connect→set + 30 s heartbeat + capped 1/2/5/10 s backoff, silent `eprintln`-only failures), `shutdown()` (stop flag + synchronous clear for real exit). Internal `Ipc` seam with real + fake adapters.
- Wiring: `mod presence` in `lib.rs`; `presence::start()` in setup (never blocks); `presence::shutdown()` in `RunEvent::Exit` only.
- Verification: `cargo check` clean (0 warnings), `cargo test` 612 passed / 0 failed (10 new presence tests), ownership gate pass. Frontend untouched. Handoff scope (prompts/skills/ai_assist parsing/fixtures) untouched — pinned qualification evidence unaffected.

## Verification

- Deterministic fake-IPC tests for set/clear/reconnect/silent-fail + payload-exactness + zero-token assertions; relevant existing backend checks/tests; frontend check/build unaffected (or run if touched); ownership gate before sync.

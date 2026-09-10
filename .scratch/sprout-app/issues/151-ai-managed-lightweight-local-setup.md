# 151 — Install an optional lightweight model and generate locally

**What to build:** Offer one qualified lightweight recommendation in managed setup, explicitly install its verified runtime and weights, generate a local draft on demand, and release owned resources when idle or quitting.

**Blocked by:** 148 — checked draft flow and qualified provider/runtime contract.

**Status:** managed integration implemented and validated fail-closed; qualification and real-model verification remain blocked by research 0018.

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md).

## ACs

- [x] Require AI setup and a separate explicit managed Install action before downloading runtime or weights. Enabling AI alone, existing-local setup and cloud setup perform no managed installation.
- [x] Build the managed integration once for compatible qualified models. Validate model-specific artifact, context/template, memory and minimum runtime requirements before activation; do not duplicate the generation pipeline for each model.
- [ ] Read the lightweight recommendation from versioned bundled JSON with exact artifact revision, source, checksum, size, license, runtime compatibility, and qualified hardware guidance. Distinguish download size from working RAM/VRAM; do not ship the proposed candidate without 146's evidence.
- [x] Fresh/AI-disabled Sprout performs no AI install, model download, recommendation refresh, or inference startup. Before an explicit install, show downloads, resource needs, source/license information, and actionable insufficient-disk/hardware errors.
- [x] Stage and verify runtime and model artifacts before activation. Cancellation, truncated/corrupt downloads, hash mismatch, and restart after interruption never expose an incomplete model as installed; retries do not damage the current usable selection.
- [x] Install per-user app-owned artifacts without administrative elevation or taking over an existing user's runtime. Start a hidden owned inference runtime only when generation needs it, with local-only access appropriate to the qualified runtime and no prompt telemetry/cloud fallback.
- [ ] Complete the end-to-end setup → Generate → checked draft → explicit Save path. Managed setup exposes the same authoring/refusal/context interface as the existing-local path, not a separate execution pipeline.
- [x] Keep a single owned runtime across overlapping requests, release model memory after a documented idle interval, and do not unload an active request. Stop the owned runtime on actual Sprout exit; main-window close while tray-resident is not exit.
- [x] Test process crashes, occupied endpoints, startup failure, cancellation/late responses, idle expiry, and normal exit with controlled process/download/clock behavior. Foreign runtimes/processes are never terminated.
- [ ] Package catalog and skill resources so an installed build needs no source checkout. Manual verification with the qualified lightweight model confirms startup, resource usage, inference locality, idle release, and continued manual action use.

## Verification

Exercise download integrity and lifecycle with fake artifacts/transports/processes and an actual qualified local runtime smoke check on supported Windows hardware. Verify no hidden console window, no duplicate owned process, no broad process-name kill, and no script execution by generation.

## Implementation notes

Model/runtime lifecycle policy lives with AI assistance; native process mechanics extend the existing Windows execution owner. Resume support is optional, but interrupted-download state and cleanup must be honest. No runtime/model license or size assumption is filled by guesswork.

## Implementation record — 2026-09-10

The managed provider now has one catalog-driven integration behind the existing
`DraftProvider` / checked-draft path. Settings exposes a separate Install
action only when both runtime and model entries are fully qualified. Artifacts
download to a per-user staging directory over HTTPS without redirects, are
size- and SHA-256-verified before atomic activation, and never replace an
existing usable revision. Generation starts one hidden loopback-only owned
runtime on demand, supports cancellation during startup and response reads,
shares it across overlapping requests, reaps it after five idle minutes, and
stops it only on actual app exit.

The shipped catalog remains intentionally non-installable. Research 0018 has
not qualified a redistributable runtime/model pair and records missing exact
revision, checksum, size, license, compatibility, hardware, and smoke-test
evidence. Consequently AC 3, the real managed portion of AC 7, and AC 10 stay
open: no candidate values were guessed, no Install button appears, and no
runtime or weights can be downloaded by this build.

Validation: `svelte-check` reported 0 errors/0 warnings; all 185 frontend tests
passed; the production frontend build passed; all 8 managed lifecycle tests
passed. The full Rust run passed 537 tests with 3 ignored and one sandboxed
registry test; that exact registry test passed when rerun with registry access.
`cargo fmt --check` could not run because this machine's only Rust toolchain
does not have the `rustfmt` component installed.

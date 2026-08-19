# 04 — Plan preview with machine detection and multi-preset composition

**What to build:** The read-only brain of the app: the `PlatformEngine` trait with a Windows detection implementation (winget list + uninstall-registry heuristics), and Plan computation for one or more selected Presets — will install / will upgrade / already OK / satisfies-by-newer / unmanaged-skip — with per-Requirement toggles, explicit conflict resolution for overlapping Products, and save-composed-as-new. This ticket makes "see exactly what will happen before anything runs" work end to end.

**Blocked by:** 02 — Preset authoring with Requirements and validation

**Status:** done — 49 backend tests green, svelte-check 0 errors, vite build ok

- [x] `PlatformEngine` trait exists with a Windows detection impl (winget list + uninstall-registry heuristics); detection is read-only and needs no elevation
- [x] Selecting one or more Presets produces a Plan: will install / will upgrade / already OK / satisfies-by-newer / unmanaged-skip per Requirement
- [x] Any Requirement can be toggled in/out for the run
- [x] Overlapping Products across selected Presets surface as explicit conflicts requiring a policy choice or opt-out — never resolved silently
- [x] A composed selection can be saved as a new Preset
- [x] Never-downgrade invariant: pinned with a newer version installed ⇒ "satisfied by newer", never a downgrade action

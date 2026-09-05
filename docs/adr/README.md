# ADRs — index

29 decisions. Original text is never rewritten; corrections live in dated
`## Amendment` sections, each file carrying a `Status` pointer up top.
Status per file: `accurate` (verified, untouched), `amended <date>`
(original preserved, correction appended).

Source audit completed 2026-09-05 for all 29 decisions: **26 amended, 3 retained**.
[Audit coverage and source evidence](AUDIT-2026-09-05.md). Amendments distinguish
current implementation from accepted obligations; no application code changed.
Historical events and operational claims were not inferred from source.

## Engine, presets, runs

- 0001 Rust/Tauri engine replaces PowerShell — `amended 2026-09-05` (Rust engine still depends on the OS PowerShell executable for selected operations)
- 0002 Single-file JSON preset — `amended 2026-09-05` (field is `schema_version`, not `schemaVersion`)
- 0003 Elevated worker self-relaunch — `amended 2026-09-05` (worker directory protocol also shares Library SQLite and submitted snapshot)
- 0004 Steps dispatch through PlatformEngine seam — `amended 2026-09-05` (PlatformEngine includes detect_many; cross-feature Windows ownership remains distributed)
- 0005 Immutable presets, run-time composition — `amended 2026-09-05` (local edits are in-place; imported snapshots normalized; merge equality is policy plus Step)
- 0007 Live-linked requirements — `amended 2026-09-05` (live references have run-start/serialization gaps; history stores outcomes, not full Requirements)
- 0009 Machine-local install directory — `amended 2026-09-05` (per-Product directory travels in local Plan/request; verify failure can erase location note)
- 0023 Plan: merge-identical, conflict-on-difference, never downgrade — `amended 2026-09-05` (merge equality narrower; pinned version/scope not passed; mixed unresolved entry gap)
- 0024 RunOutcome dominance + honesty notes — `amended 2026-09-05` (cancellation overrides prior failures; notes and verification trailers are incomplete)
- 0029 One source of truth per Windows command — `new 2026-09-05` (accepted ownership constraint; implementation candidates under review)

## Distribution, data, portability

- 0006 Lazy data under %LOCALAPPDATA% — `amended 2026-09-05` (installer effects and optional local/roaming removal clarified; re-diff provenance unverified)
- 0008 No first-run seed — `amended 2026-09-05` (seed file absent; backup restore also creates deliberate Product data)
- 0012 Self-update from GitHub Releases (+ ed25519 scheme B) — `amended 2026-09-05` (two install affordances; signature encoding supported; custody remains operational obligation)
- 0013 Boot to tray with dock restore — `amended 2026-09-05` (dock restore does not cover every main-window entry point or restore a saved monitor)
- 0014 One backup document format — `amended 2026-09-05` (zero selected collections differs from zero records; Preset payloads bypass validation)
- 0025 Logs expire, history is forever, run-active from disk — `amended 2026-09-05` (pruning triggers narrowed; completion dedup is frontend-local; durability qualified)
- 0026 Machine-local boundary + backup identities + ordered lists — `amended 2026-09-05` (five-collection backup scope; paths/desktop assignments retained; actual dedup and ordering)

## Quick access (tray, window, dock, lists)

- 0010 Tray-resident lean backend, cap+queue — `amended 2026-09-05` (tray opens window; basename matching and queue-slot/close semantics clarified)
- 0011 Quick Launch window + AppBar dock — `amended 2026-09-05` (auto-hide requests zero-width ABM_SETPOS; prior tab and floating corrections hold)
- 0015 Virtual-desktop assignments activate by use — `accurate`
- 0016 Groups: isolated namespaces, dissolve, dormant-off — `amended 2026-09-05` (create/assign is not atomic; off hides grouping but lifecycle maintenance continues)
- 0017 Quick Action execution model — `amended 2026-09-05` (inherited token; optional logs; watchdog tracks original action tree only)
- 0018 Launch pipeline: cap+queue, honest outcomes — `amended 2026-09-05` (basename matching; foreground outcome and Store already-open detection gaps)
- 0019 Dock autohide: driver owns motion — `amended 2026-09-05` (driver owns animation, not every geometry write; workspace release is fallible)
- 0020 Monitor identity + seam eligibility — `amended 2026-09-05` (duplicate native identity probes; opposite-edge migration is not revalidated)
- 0021 Single size source + dock-width math — `amended 2026-09-05` (Settings duplicates width formula/constants; floor priority and logical/physical units)
- 0022 Companion: one isolated site, docked only — `amended 2026-09-05` (whole-URL dedup; best-effort mute application and explicit external-opening scope)
- 0027 App discovery snapshot; winget authoring-only — `amended 2026-09-05` (backend freshness vs retained picker snapshot; dedup, registry fallback, and icon behavior)

## Frontend system

- 0028 Design system + disclosure rules — `amended 2026-09-05` (design-system rule remains required; literal values and contrast gate are implementation gaps)

## Adding one

New ADR only when all three hold: hard to reverse, surprising without
context, a real trade-off was picked. Number = highest + 1. Never rewrite a
file's original text — append a dated `## Amendment`.

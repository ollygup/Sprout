# ADRs — index

28 decisions. Original text is never rewritten; corrections live in dated
`## Amendment` sections, each file carrying a `Status` pointer up top.
Status per file: `accurate` (verified, untouched), `amended <date>`
(original preserved, correction appended).

## Engine, presets, runs

- 0001 Rust/Tauri engine replaces PowerShell — `amended 2026-09-05` (inbox powershell.exe still spawned; "no runtime" = no 5.1/contract)
- 0002 Single-file JSON preset — `amended 2026-09-05` (field is `schema_version`, not `schemaVersion`)
- 0003 Elevated worker self-relaunch — `accurate`
- 0004 Steps dispatch through PlatformEngine seam — `amended 2026-09-05` (registry design never existed; seam + adapter stated)
- 0005 Immutable presets, run-time composition — `amended 2026-09-05` (fork is UI-enforced; identical declarations merge silently, conflicts never do)
- 0007 Live-linked requirements — `accurate`
- 0009 Machine-local install directory — `amended 2026-09-05` (per-product override shipped; portability by stripping)
- 0023 Plan: merge-identical, conflict-on-difference, never downgrade — `new 2026-09-05`
- 0024 RunOutcome dominance + honesty notes — `new 2026-09-05`

## Distribution, data, portability

- 0006 Lazy data under %LOCALAPPDATA% — `amended 2026-09-05`; **standing obligation: re-diff vendored NSIS template (baselines 2.9.4, lockfile has Tauri 2.11.5)**
- 0008 No first-run seed — `amended 2026-09-05` (orphan `seed.rs` still on disk; deletion is a git-side job)
- 0012 Self-update from GitHub Releases (+ ed25519 scheme B) — `accurate`
- 0013 Boot to tray with dock restore — `amended 2026-09-05` (no primary-monitor branch; hide is off-screen, no sliver)
- 0014 One backup document format — `accurate`
- 0025 Logs expire, history is forever, run-active from disk — `new 2026-09-05`
- 0026 Machine-local boundary + backup identities + ordered lists — `new 2026-09-05`

## Quick access (tray, window, dock, lists)

- 0010 Tray-resident lean backend, cap+queue — `amended 2026-09-05` (gate is major build 26100; second-Start is a command error)
- 0011 Quick Launch window + AppBar dock — `amended 2026-09-05` (3 tabs; action rows are Run/Stop; floating never remembers geometry)
- 0015 Virtual-desktop assignments activate by use — `accurate`
- 0016 Groups: isolated namespaces, dissolve, dormant-off — `new 2026-09-05`
- 0017 Quick Action execution model — `new 2026-09-05`
- 0018 Launch pipeline: cap+queue, honest outcomes — `new 2026-09-05`
- 0019 Dock autohide: driver owns motion — `new 2026-09-05`
- 0020 Monitor identity + seam eligibility — `new 2026-09-05`
- 0021 Single size source + dock-width math — `new 2026-09-05`
- 0022 Companion: one isolated site, docked only — `new 2026-09-05`
- 0027 App discovery snapshot; winget authoring-only — `new 2026-09-05`

## Frontend system

- 0028 Design system + disclosure rules — `new 2026-09-05` (tokens/components sole source; research 0004–0008 rules locked)

## Adding one

New ADR only when all three hold: hard to reverse, surprising without
context, a real trade-off was picked. Number = highest + 1. Never rewrite a
file's original text — append a dated `## Amendment`.

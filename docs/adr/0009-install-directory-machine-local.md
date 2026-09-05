# Install directory — machine-local global default (ticket 34)

> Status: amended 2026-09-05 — original text preserved below; the correction is in the Amendment section.

A Settings value, `settings.install_dir`, names the directory installs and upgrades should land in. Empty means winget's own default. Runs pass it to winget as `--location`, the Plan says where software will go, and run results report where it actually landed — calling out installers that ignore the request. The value is machine-local: it is never part of a Preset, a Plan payload, a Run record, or an export.

## Why

Software installs scatter across `C:\Program Files`, per-user folders, and drive roots. A user who keeps a second drive for applications — the reason this ticket exists — wants one knob that steers every install to `D:\Apps`, without editing presets. Winget supports `--location` on `install`/`upgrade` but honors it only when the installer itself supports a custom directory; several do not. Honesty about that is the second half of the feature: the run must say where software actually landed when it ignored the request.

## Decisions

- **A Settings value, not a Preset or Product field**: the directory is a property of the machine, not of the intent. Presets stay declarative and portable; a per-product override is a later, additive change (ticket 36, blocked by this ADR).
- **Empty is the winget default**: no directory set — no `--location` flag — the installers decide. There is no synthetic default that could surprise.
- **Validated as an absolute Windows path**: drive-rooted (`D:\Apps`) or UNC. Relative paths are rejected by `Settings::validate`, and an unreadable value in the database loads as empty rather than failing the app.
- **The worker reads it at run time**: the elevated worker loads Settings from the database best-effort when a run starts and passes the directory into the engine. The Plan (a machine-agnostic composition) never carries it; the UI renders "installs go to …" from Settings.
- **`--location` on install and upgrade**: the Windows engine appends `--location <dir>` to both verbs; prepare, verify, env wiring, and checks are untouched.
- **Post-run honesty**: after a successful install/upgrade, when a directory was requested, the engine resolves where the product actually landed from its install-location hint (registry). If it resolves and differs from the request (case-insensitive, trailing-separator-blind), the run detail appends `installed to {actual} (installer ignored the requested directory)`. A matching location, an unresolvable hint, or a failed step produce no such note.
- **Never exported**: the directory is not in the domain model, so preset export cannot leak it; a regression test pins that down.
- **Not part of the Run record**: history replays outcomes, and the honesty note lives in the outcome detail, which persists. The directory itself is not persisted per run.

## Consequences

- Presets, Plan payloads, and `.sprout.json` files remain portable across machines with different layouts.
- Users with installers that ignore `--location` see exactly where each product landed, in the run results on the Plan and in History.
- The Plan's "Ready to apply" group and summary name the target directory when one is set, so the machine's intent is visible before anything runs.
- Ticket 36 (per-product override) extends this design: a per-product directory wins over the global default in the run pipeline; exports still never carry either.

## Amendment — 2026-09-05 (codebase accuracy pass)

Ticket 36 has shipped, so the future-tense above is now present-tense: `Product.install_dir` (`domain.rs`) is a machine-local override of the global default, resolved per Requirement as `product.install_dir.or(global)` in `run.rs` and passed into the engine seam for both install and upgrade. The "not in the domain model" rationale in the Decisions section is superseded — the field IS in the model now — but the portability guarantee holds by explicit stripping instead: preset export/import strip it (`import_export.rs` both directions) and whole-app backup strips it both ways (`backup.rs` normalize). The Run record still carries no directory column; the honesty note lives in the outcome detail.
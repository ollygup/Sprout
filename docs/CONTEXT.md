# Sprout

A Windows desktop app for composing, running, and sharing software-installation presets. Replaces the legacy PowerShell setup package, which passed the release parity gate and was removed at v1 (see `docs/release/parity-checklist.md`).

## Language

**Product**:
A thing installable on this machine — winget ID, display name, install-location hint, and default env suggestions.
_Avoid_: App, package, catalog entry

**Requirement**:
A declaration that the machine must have a specific Product in a specific state: a VersionPolicy, an optional Step, optional Env wiring, and optional verify commands. In presets created or forked in Sprout, the Requirement is a **live reference** to the Library Product by id — the name and winget step are resolved from the Library at plan and run time (ADR-0007). Imported presets keep their authored snapshot instead. A requirement whose product left the Library is "removed from library" (unresolved) and excluded from runs until the product is re-added. In the composer the Requirements of a preset are presented as **Applications** (a UI synonym, ticket 35) — the data shape stays `Requirement` end to end.
_Avoid_: Item, config entry, app entry

**VersionPolicy**:
How the machine must relate to a Product's version: `latest` (upgrade to newest), `pinned` (exact version), or `present` (installed, never upgraded).
_Avoid_: Version constraint, update behavior

**Step**:
The mechanism by which a Requirement is executed — `winget` or `command` — described as data with executor-specific parameters.
_Avoid_: Installer, script

**Preset**:
A named, versioned, exportable set of Requirements targeting a platform. The unit of sharing; immutable once imported, edited only by forking.
_Avoid_: Profile, configuration, setup

**Preset file**:
The `.sprout.json` file a Preset is exported to. Double-clicking one opens Sprout and imports it (registered by the installer via the Windows file association).
_Avoid_: Setup file, config file

**Plan**:
The computed expected actions for one or more Presets on this machine (will install / will upgrade / already OK / satisfies-by-newer / conflict), produced read-only before anything runs.
_Avoid_: Dry run, preview

**Run**:
One application of a Plan to this machine, stored with per-Requirement outcomes and log file paths.
_Avoid_: Session, install job

**Outcome**:
The overall verdict of a Run, derived from its per-Requirement results: *Applied* (everything applied or was already satisfied — nothing needed attention), *With notes* (the run completed but something needed attention, e.g. an unmanaged product was detected and skipped), *Cancelled* (the user aborted between Requirements), or *Failed* (a Requirement failed or timed out). A run is only ever "clean" when nothing needed attention.
_Avoid_: Status, result

**Env wiring**:
The `set` / `prepend` environment-variable operations a Requirement applies after a successful install. User scope only; never overwrites existing values; `<InstallLocation>` resolved from the uninstall registry at apply time.
_Avoid_: Environment variables (when referring to the whole mechanism)

**Library**:
The user's local collection of Presets and Products, stored in the local database. Products are the source of truth for requirement names and winget steps; deleting one removes it from the presets that reference it (local presets drop the requirement, imported presets keep their snapshot).
_Avoid_: Database, catalog

**Install location hint**:
A Product-level hint used to find where software landed after an install (e.g. a needle against the uninstall registry), also backing the `<InstallLocation>` env placeholder. It describes the product, not a policy — it never requests a directory.
_Avoid_: Install path, target directory

**Install directory**:
The machine-local Settings value (`settings.install_dir`) that names where installs and upgrades should land — empty means winget's own default. Runs pass it to winget as `--location`; the Plan shows "installs go to …"; a run result that lands elsewhere reports it ("installer ignored the requested directory"). Never part of a Preset, Plan payload, or export (ADR-0009). Per-product overrides are future work (ticket 36).
_Avoid_: Install location hint (a Product property, not a setting), install path

**Verify command**:
A command declared on a Requirement and run after install; a non-zero exit or non-matching output fails the Requirement.
_Avoid_: Post-install check

# 02 — Preset authoring with Requirements and validation

**What to build:** Authoring a Preset in the app: create a named Preset, add Requirements (Product + VersionPolicy `latest`/`pinned`/`present`, dependsOn, timeout, env wiring entries, verify commands), save with validation, and manage it in the Library. This ticket makes "I configured my setup in the app" work end to end.

**Blocked by:** 01 — App scaffold with seeded Product library

**Status:** done

- [x] Create a named Preset with a description; it appears in the Library
- [x] Add Requirements to a Preset: Product + VersionPolicy (latest | pinned with a version | present), dependsOn, timeout, env wiring entries (set/prepend grammar), verify commands
- [x] Save validates: duplicate Product within one Preset, unknown dependency, malformed policy or env entry → clear error, nothing saved
- [x] Edit and delete Presets; fork (copy) a Preset into a new editable one

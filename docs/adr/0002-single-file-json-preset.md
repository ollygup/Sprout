# Presets are single-file JSON with a schema version

> Status: amended 2026-09-05 — original text preserved below; the correction is in the Amendment section.

Exported presets are one self-contained `.sprout.json` — `schema_version`, metadata, and full product definitions embedded — so the file is the unit of sharing and needs no knowledge of the recipient's library or Sprout version. JSON over YAML because the backend is serde-native, validation is strict, and the file stays diffable in git; YAML would add a parser and looser semantics for no gain. `schema_version` guards future format changes: too-new or too-old files are rejected with a clear message instead of being misread.

## Amendment — 2026-09-05 (codebase accuracy pass)

Field-name correction only: the ADR originally wrote `schemaVersion` (camelCase); the format has always been `schema_version` (snake_case) in code and on disk (`domain.rs`, `parity-preset.sprout.json`). Validation behavior is as described — only version 1 is accepted, both directions rejected with an explicit message. File-association / double-click import behavior unchanged.

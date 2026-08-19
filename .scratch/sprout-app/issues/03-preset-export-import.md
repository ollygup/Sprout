# 03 — Preset export/import as self-contained .sprout.json

**What to build:** Moving Presets between machines: export any Preset to a single self-contained `.sprout.json` (schemaVersion 1, platform, metadata, embedded product definitions), and import a file into the Library immutably — via an in-app dialog and via launch-with-file-path (so double-click works once the association is registered). This ticket makes the full export → send → import round-trip work end to end.

**Blocked by:** 02 — Preset authoring with Requirements and validation

**Status:** done

- [x] Export any Preset to a single `.sprout.json`; re-importing that file reproduces the Preset exactly
- [x] Import via in-app file dialog and via launching with a file path argument
- [x] Imported Presets are stored immutable (fork is required to edit)
- [x] Clear rejection messages: unsupported schemaVersion, wrong-platform warning, duplicate product within the file

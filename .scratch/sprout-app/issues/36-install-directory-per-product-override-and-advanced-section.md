# 36 — Install directory: per-product override + Advanced section

**What to build:** Each Product can declare its own install directory, overriding the global default; the Plan and runs honor the override. The add/edit product dialog is reworked: basic fields are Name and winget ID, and an Advanced section holds the install location hint, default env wiring, and the install directory — with the caveat that many installers ignore location flags. Cards and the More info dialog show the override.

**Blocked by:** 34 — Install directory: global default

**Status:** done

- [x] Product CRUD persists an install directory; existing databases migrate cleanly (idempotent column add)
- [x] Plan-time precedence: product override wins, global default applies otherwise
- [x] Product dialog: basic section = Name + winget ID (search-first); Advanced section holds install location hint, default env wiring, and install directory with the caveat "Many installers ignore location flags so it does not guarantee installation in a specified drive"
- [x] Card shows the override under the winget id (e.g. `dir: D:\Apps`); the More info dialog shows it
- [x] The directory is stripped from preset payloads and exports (never shared)
- [x] Backend tests: migration, CRUD, precedence, stripping
- [x] `npm run check` 0 errors
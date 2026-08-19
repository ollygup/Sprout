# 34 — Install directory: global default

**What to build:** Settings gains a "Default install directory" control with a folder picker and a clear option. When set, winget install and upgrade commands carry `--location`. The Plan shows the target directory per application. After a run, each application's result reports where it actually landed and calls out when the installer ignored the requested directory. The directory never appears inside exported preset files. An ADR records install directories as machine-local.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] Settings persists a default install directory (empty = winget default); non-empty values validated as absolute paths
- [x] Plan rows show the target directory when a default is set ("install to D:\Apps" or equivalent)
- [x] The winget engine appends `--location` for both install and upgrade when a directory applies
- [x] After a successful install with a requested directory and a resolvable registry hint, the requirement detail reports the actual location and flags a mismatch ("installed to C:\Program Files\… (installer ignored the requested directory)"); no fabricated note when the hint is missing
- [x] Exported `.sprout.json` files never contain the directory
- [x] Backend tests: settings roundtrip/validation, `--location` arg assembly, payload stripping, detail-line behavior
- [x] ADR-0009 written; CONTEXT.md gains the "Install directory" term, distinct from "Install location hint"
- [x] `npm run check` 0 errors

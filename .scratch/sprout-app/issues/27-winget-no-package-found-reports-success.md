# 27 — winget "no package found" is reported as success (whitelist swallows the real exit code)

**What to build:** Installing a Product whose winget ID does not exist must fail honestly. Today the run log contains winget's raw `No package found matching input criteria.` yet History and the run-completion popup report success (Installed / Applied). Root cause found: for a nonexistent ID, winget exits `-1978335212` = `0x8A150014` (NO_APPLICATIONS_FOUND), and that code sits in `classify_winget_result`'s success whitelist (`src-tauri/src/engine/windows.rs`) mapped to "already up to date" — a semantic meant for `winget upgrade` ("no newer package in source") that also swallows the install-of-missing-package case. The ticket-16 tests missed it because they exercise a fake exit code 5, never the real `0x8A150014`.

**Blocked by:** — (bug found in ticket-16 follow-up; backend only)

**Status:** done

- [x] `classify_winget_result` returns `None` (genuine failure) when the output carries the "no package found matching" message — for both install and upgrade, before any whitelist match — so `winget_failure_detail`'s honest wording ("can't find this app in the winget registry — check its ID") is what surfaces
- [x] The `0x8A150014` whitelist arm applies to `upgrade` only (defense in depth for output-less runs)
- [x] Regression tests in `windows.rs`: `classify_winget_result("install", 0x8A150014 as i32, "No package found matching input criteria.") == None`; same for `upgrade`; `upgrade` + `0x8A150014` + empty output still whitelisted ("already up to date"); existing benign-code tests untouched
- [x] Live feedback loop confirmed before/after: `winget install --id Sprout.Does.Not.Exist --source winget --accept-source-agreements --accept-package-agreements --disable-interactivity` exits `-1978335212` (0x8A150014) with the message — RED before fix (classified success), GREEN after (Failed with the check-your-ID message)
- [x] `cargo test` green in `src-tauri\`, `npm run check` 0 errors; working copy synced to the share (add/update robocopy, `/L` shows Copied: 0)

## Diagnosis record (ticket 27, 2026-08-16)

### Symptom (user)

Installing a product whose winget ID does not exist: the per-requirement log contains `No package found matching input criteria.` but History and the completion popup say the run succeeded.

### Root cause

The live probe (read-only winget invocation of a guaranteed-nonexistent ID) reproduced the exact symptom and pinned the exit code: `-1978335212` = `0x8A150014`. In `classify_winget_result`, `match exit_code as u32` hits the `0x8A15_0014` arm → `WingetReason { reboot: false, detail: "already up to date" }` → `StepOutcome { ok: true }` → `RunStatus::Installed` → `RunOutcome::Ok`. The whitelist arm's comment ("NO_APPLICATIONS_FOUND (no newer package in source)") documents the upgrade semantic; winget reuses the same code for the install-missing-package case, and the message backstop never runs because the whitelist match wins.

### Evidence chain

- Probe exit `-1978335212`, message `No package found matching input criteria.` — the user's exact symptom reproduced in one command
- `0x8A150014` resolves as `u32` to the whitelisted arm; no "already installed" / "no update" backstop phrases in the output
- Ticket-16 test `no_package_found_names_the_stale_id_cause` passes only because it uses exit code 5 — the real code path was never covered

### Fix direction (agreed at diagnosis)

1. Message guard at the top of `classify_winget_result`: `no_package_found(&joined) → return None`.
2. Gate the `0x8A15_0014` arm on `action == "upgrade"`.
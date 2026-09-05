# Windows operation ownership

Structural implementation of ADR-0029; all four candidates authorized 2026-09-05.
Keep PlatformEngine, LauncherEngine, catalog commands, and product behavior stable.
Behavior gaps from the ADR audit are separate work.

## Session stopping point — 2026-09-05

User requested stopping after Windows execution and continuing the other two
candidates in another session. No inspection or display application code was
changed. The original temporary handoff.md was updated for continuation.
The read-only independent review found no actionable winget/execution regressions.
Migration scripts under .scratch were development artifacts, not runnable product
code: refactor-execution.cjs ran and refactor-inspection.cjs stayed an
unexecuted partial draft; both were deleted after use.

For inspection, preserve first visible/ownerless PID match (including blank-title
or shell windows), while general snapshots exclude blank titles and shell chrome.
Keep PID, unseen image, unseen AUMID, then unseen direct-child resolution order.
For displays, preserve last successful target in the map versus first successful
matching target in identity lookup, including first zero EDID returning None.
Target query failures must not overwrite a successful map entry.

- [x] Consolidate winget authoring, detection, mutation, and bootstrap ownership.
- [x] Consolidate Windows execution and shell activation mechanics.
- [x] Consolidate Quick Launch native window/process inspection.
- [x] Consolidate native display probing with distinct selection policies.
- [x] Finish operation inventory and final validation.

## Validation

Baseline: frontend check 0 errors/warnings; backend 440 passed, 1 failed,
3 ignored under sandbox. The registry environment round-trip test passed
with normal registry access, identifying the sandbox restriction.

Winget: existing parser, classification, preparation, and run tests retained;
three new transcript tests exercise arguments, timeouts, catalog versus mutation
exit policies, logs, missing packages, and failed detection reads. Full backend
suite passed with normal registry access (444 passed, 3 ignored).

 Windows execution: full backend suite passed (447 passed, 3 ignored).
 Shell tests exercise ordinary open versus hidden elevated worker intent and
 native result translation without launching either. Existing cwd, capture,
 timeout, action run/stop and watchdog tests remain; a clone-failure test keeps
 required run-log attachment distinct from best-effort stop-log attachment.
 The existing greater-than-32 success rule makes the worker's 1223 error arm
 unreachable; this structural change preserves it.

 Inspection: `engine/windows.rs` keeps the LauncherEngine seam and now owns
 only spawn/foreground/desktop-move; all EnumWindows/process-handle/target
 resolution lives in private `engine/windows/inspection.rs` behind a
 `WindowSource` seam (production `NativeWindows`, exercised `StubWindows`
 record adapter). Preserved: first visible/ownerless PID match (blank-title
 and shell-chrome windows included) versus snapshot exclusion of blank titles
 and shell chrome; PID, unseen image, unseen AUMID, then unseen direct-child
 order; UWP-first matching in `app_windows`. Eight new tests lock the order,
 the snapshot/PID split, AUMID/basename preference, target-exists cases, and
 the lnk raw-bytes fallback (moved with its implementation). Full backend
 suite passed after the move (454 passed, 3 ignored).

 Display: `appbar.rs` keeps enumeration composition, monitor accessors, and
 all dock geometry; the `QueryDisplayConfig` snapshot, EDID normalization,
 and header construction live in private `appbar/display.rs` behind a
 `DisplayQuery` seam (production `NativeDisplayQuery`, exercised
 `StubDisplayQuery` record adapter). Preserved: last successful target wins
 in the map, failed target queries never overwrite a successful map entry
 (failure with no prior success still records the source), first matching
 target wins in identity lookup including first zero EDID returning `None`,
 failed targets skipped in lookup. Seven tests lock the split (map-last vs
 lookup-first on the same records, failure preservation, zero-EDID, empty
 sources, missing devices) plus the moved EDID/wide-match tests. Full
 backend suite passed after the move (459 passed, 3 ignored).

## Operation inventory

| Operation | Owner | Preserved distinction |
| --- | --- | --- |
| winget version/list | winget.rs | unbounded capture, failed read fallback |
| winget search/show | winget/authoring.rs | authoring only, timeboxed, nonzero fails |
| winget install/upgrade | winget/mutation.rs | exact/source/location flags, exit/reboot whitelist |
| winget bootstrap | winget/bootstrap.rs | build gate, release/download/install, cleanup |
| timed capture / taskkill tree | windows_execution/process.rs | polling, stderr prefix, timeout drain |
| PowerShell invocation / action run and stop | windows_execution/process.rs | inherited token, hidden, cwd, different log failure policies |
| ShellExecuteW | windows_execution/shell.rs | normal visible open versus hidden runas worker |
| window/process inspection | engine/windows/inspection.rs | pid-first match vs snapshot exclusion; pid/image/AUMID/child order |
| window target resolution | engine/windows/inspection.rs | IShellLink then raw-bytes lnk fallback, bare-name PATH rule |
| display-config probe | appbar/display.rs | one snapshot impl; map last-success vs lookup first-match policies |
| EDID normalization | appbar/display.rs | hex identity, wide-string match, zero-EDID rule |

ShellExecuteExW/Store activation remain distinct operations in the Windows
launch adapter. Update detachment remains in update.rs. No universal dispatcher
or PlatformEngine/LauncherEngine method was added.

Deletion test: removing the winget module would redistribute executable,
argument, parsing, and compatibility knowledge across catalog and install
callers. The private process seam has a production adapter and an exercised
transcript adapter. Its depth comes from interpreting operations, not file size.

## Enforcement

`tools/ownership-gate.mjs` checks this inventory on every unit of work and
session end (AGENTS.md: must pass before `-Up`). A push-time workflow was
considered and deliberately removed: in this repo every tree reaching git
has already passed the session gate, so it could only re-confirm. The gate
scans `src-tauri/src/**/*.rs` for each owned native symbol, its import path,
and `as`-aliases, skipping comment lines — any reference outside the owner
file fails. `taskkill` and the `winget` executable name are deliberately
untabled (test cleanup and log strings would false-positive); their single
production owners (`kill_tree`, the winget module) stand by review. Release
protection, if wanted, belongs as a pre-build step in `release.yml`
(proposed, not implemented).

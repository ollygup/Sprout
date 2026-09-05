# App discovery is a fresh snapshot; winget is authoring-time read-only

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

The installed-app picker is a fresh snapshot on every call — no cache. It merges three Windows sources: Start Menu shortcuts (resolved links, Store apps excluded), the uninstall registry hives (64-bit, 32-bit, and per-user, with system components, parented entries, and AppX-uninstall rows filtered; `DisplayIcon` resolving to an exe wins, else a bare exe under the install location), and Store/MSIX packages (framework, resource, bundle, and dev-mode packages filtered, with a PowerShell fallback when the native API is unavailable). Dedup favors truth over cleverness: Start Menu wins on exe path and donates the publisher; Store entries dedup AUMID-exact; Win32 and Store never merge on display name alone. Launch keys are launchable (`shell:AppsFolder\AUMID` for Store, shortcut or exe otherwise). Icons are memory-only 32 px data URLs with a packaged-logo fallback — never written to disk (a memory-only frontend cache holds data URLs for the session, so nothing persists past it). `winget search`/`show` is strictly authoring-time and read-only: it fills the Add-product picker (timeboxed, 20 rows, aligned-table parsing with an id-anchored localized fallback) and never participates in detection, planning, or runs.

## Consequences

- The picker can be slow but never stale; a cache would trade correctness for speed in exactly the place users notice ghosts.
- Installing from the picker is out of scope — discovery names a target, Presets declare intent, runs execute it. Those are three different jobs.

## Amendment — 2026-09-05 (executable-source audit)

The backend freshness claim holds for each `walker::snapshot` invocation (`src-tauri/src/walker.rs`), but not for every opening of the picker: `toggleAdd` in `src/routes/+page.svelte` calls `loadCandidates` only while `candidatesLoaded` is false. The mounted page retains its candidate array, so reopening the panel can show an older snapshot. Successful icon results are likewise cached for the webview lifetime by `fetchIcon` in `src/lib/lazyIcon.svelte.ts`. Fresh enumeration remains the decision; the stronger claim that the picker can never be stale is not an implemented guarantee.

The executable discovery rules are more specific than the original paragraph:

- `collect_lnks` excludes links whose resolved target contains `WindowsApps`; unresolved links remain as shortcut candidates. `candidate_from_registry` uses `DisplayIcon` when it parses as an executable path, otherwise accepts `InstallLocation` only when that value itself ends in `.exe`. It does not search a directory for an executable or prove the path still exists.
- `merge_three` preserves the Start Menu target on an executable-path match and fills a missing publisher from the registry candidate. The registry donates the publisher, not the Start Menu. Before path/AUMID merging, `collapse_by_name` runs independently on all three sources, keyed by lowercased display name with resolved-path preference. Thus Store records can collapse before the case-insensitive AUMID pass; distinct AUMIDs are not unconditionally preserved. That is a gap against the stated identity rule, not a decision to replace it. Win32 and Store candidates still do not merge on display name across sources.
- `icons::candidate_icon` renders ordinary shell icons at 32 px. Store targets first use their packaged logo PNG at its source dimensions, then shell extraction on that logo file, then no icon. Packaged logos are the first choice for Store targets, not a universal fallback behind a 32 px icon. The generated data URLs remain memory-only.
- `winget::search` owns the 20-result cap and aligned/localized search parsing; `winget::show` returns one parsed detail record. Their authoring-only use is unchanged.

This amendment records the current implementation and its gaps; it does not authorize changing discovery, cache, or identity semantics.

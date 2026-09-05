# App discovery is a fresh snapshot; winget is authoring-time read-only

The installed-app picker is a fresh snapshot on every call — no cache. It merges three Windows sources: Start Menu shortcuts (resolved links, Store apps excluded), the uninstall registry hives (64-bit, 32-bit, and per-user, with system components, parented entries, and AppX-uninstall rows filtered; `DisplayIcon` resolving to an exe wins, else a bare exe under the install location), and Store/MSIX packages (framework, resource, bundle, and dev-mode packages filtered, with a PowerShell fallback when the native API is unavailable). Dedup favors truth over cleverness: Start Menu wins on exe path and donates the publisher; Store entries dedup AUMID-exact; Win32 and Store never merge on display name alone. Launch keys are launchable (`shell:AppsFolder\AUMID` for Store, shortcut or exe otherwise). Icons are memory-only 32 px data URLs with a packaged-logo fallback — never written to disk (a memory-only frontend cache holds data URLs for the session, so nothing persists past it). `winget search`/`show` is strictly authoring-time and read-only: it fills the Add-product picker (timeboxed, 20 rows, aligned-table parsing with an id-anchored localized fallback) and never participates in detection, planning, or runs.

## Consequences

- The picker can be slow but never stale; a cache would trade correctness for speed in exactly the place users notice ghosts.
- Installing from the picker is out of scope — discovery names a target, Presets declare intent, runs execute it. Those are three different jobs.

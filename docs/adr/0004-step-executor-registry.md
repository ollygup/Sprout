# Step types dispatch through an executor registry

Steps are data and execution dispatches through `HashMap<StepType, Box<dyn StepExecutor>>` — the plugin-registry pattern Tauri itself uses — so the run loop never hardcodes step logic and new step kinds are additive registrations, not invasive changes. v1 ships `winget` and `command` step types. Winget's manifest system already covers exe/msi/msix/zip-portable installs, silent switches, and success codes, so most installer shapes are winget cases; a future download-and-run type is one new executor struct plus one registry entry.

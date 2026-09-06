# Cleanup — agent reference

> Read this file when: you are told to do a cleanup, or automatically after
> any local `tauri build`. Otherwise skip it.

- WHEN told to do a cleanup, AND automatically after any local `tauri build` → MUST run cleanup. Device-only — the share never holds these dirs; nothing here is deleted from the repo or the share.
  - MUST run `Remove-Item -Recurse -Force C:\Sprout\src-tauri\target` — the big one (often several GB: debug builds + incremental caches). Recreated by the next `tauri dev` / `cargo test` / `tauri build`.
  - MUST leave `node_modules` and `%LOCALAPPDATA%\Sprout` alone (needed constantly / user data). `.svelte-kit`, `build\`, `src-tauri\gen` are tiny — optional.

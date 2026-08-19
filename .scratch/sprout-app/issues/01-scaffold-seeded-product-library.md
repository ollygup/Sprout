# 01 — App scaffold with seeded Product library

**What to build:** A running Sprout app on first launch: the Tauri 2 + Svelte 5 skeleton, lazy data initialization under `%LOCALAPPDATA%\Sprout`, the domain model types, the 14 seeded Products (the current catalog entries), and a searchable Library view of Products built on the design-token/component foundation. This ticket makes "open the app and see what's available to install" work end to end.

**Blocked by:** None — can start immediately

**Status:** done

- [x] `npm run tauri dev` launches a window; first run creates `%LOCALAPPDATA%\Sprout\sprout.db` and the logs directory lazily — nothing is created before first launch
- [x] The Library view lists the 14 seeded Products (name, winget ID) from the current catalog, searchable
- [x] Products are editable and deletable; changes persist across restarts
- [x] The design-token/component foundation (palette, type, accessible components) is in place and used by the Library view
- [x] The project layout exists with the Rust engine behind the platform strategy seam (trait stubbed for later tickets)

import path from "node:path";
import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

/// The composer-state tests (ticket 35) run in Node — the module under test
/// is a plain `.svelte.ts` (runes) with no DOM or Tauri dependencies, so only
/// the svelte plugin is needed to compile it. The theme-system tests import a
/// `.svelte.ts` module that imports `$lib/api`, so the kit alias is mirrored
/// here for the Node environment.
export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: { $lib: path.resolve(import.meta.dirname, "src/lib") },
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
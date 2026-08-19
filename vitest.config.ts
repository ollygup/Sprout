import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

/// The composer-state tests (ticket 35) run in Node — the module under test
/// is a plain `.svelte.ts` (runes) with no DOM or Tauri dependencies, so only
/// the svelte plugin is needed to compile it.
export default defineConfig({
  plugins: [svelte()],
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
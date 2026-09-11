import path from "node:path";
import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

/// The composer-state tests (ticket 35) run in Node — the module under test
/// is a plain `.svelte.ts` (runes) with no DOM or Tauri dependencies, so only
/// the svelte plugin is needed to compile it. The theme-system tests import a
/// `.svelte.ts` module that imports `$lib/api`, so the kit alias is mirrored
/// here for the Node environment.
/// Default-exclude set, repeated per project: a project-level `exclude`
/// replaces (not extends) the defaults, so each project restates them.
const DEFAULT_EXCLUDES = [
  "**/node_modules/**",
  "**/dist/**",
  "**/cypress/**",
  "**/.{idea,git,cache,output,temp}/**",
  "**/{karma,rollup,webpack,vite,vitest,jest,ava,babel,nyc,cypress,tsup,build}.config.*",
];

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: { $lib: path.resolve(import.meta.dirname, "src/lib") },
  },
  test: {
    include: ["src/**/*.test.ts"],
    // Two projects: pure-logic tests stay in Node exactly as before, while
    // interaction tests mount real components and need DOM globals plus the
    // browser build of the svelte runtime.
    projects: [
      {
        plugins: [svelte()],
        resolve: {
          alias: { $lib: path.resolve(import.meta.dirname, "src/lib") },
        },
        test: {
          name: "unit",
          environment: "node",
          include: ["src/**/*.test.ts"],
          exclude: [...DEFAULT_EXCLUDES, "src/lib/*.close.test.ts"],
        },
      },
      {
        plugins: [svelte()],
        resolve: {
          conditions: ["browser", "node"],
          alias: {
            $lib: path.resolve(import.meta.dirname, "src/lib"),
            "$app/navigation": path.resolve(import.meta.dirname, "node_modules/@sveltejs/kit/src/runtime/app/navigation.js"),
          },
        },
        test: {
          name: "dom",
          environment: "happy-dom",
          include: ["src/lib/*.close.test.ts"],
          exclude: DEFAULT_EXCLUDES,
        },
      },
    ],
  },
});

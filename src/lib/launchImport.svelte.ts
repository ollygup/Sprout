/// The `.sprout.json` path the app was launched with (double-click / file
/// association). Consumed by the Presets page once the user is looking at it.
export const launchImport = $state<{ path: string | null }>({ path: null });
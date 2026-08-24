/** The five content collections a backup carries, with the display names
 *  every surface shares: nav-rail tabs and the backup export picker all
 *  read from here, so one collection never carries two names. `one`/`many`
 *  are the count nouns behind the backup notices ("3 products, 1 preset"). */
export const COLLECTIONS = {
  launch_entries: {
    label: "Quick Launch",
    one: "launch entry",
    many: "launch entries",
  },
  quick_actions: {
    label: "Quick Actions",
    one: "quick action",
    many: "quick actions",
  },
  clips: { label: "Quick Clips", one: "clip", many: "clips" },
  products: { label: "Products", one: "product", many: "products" },
  presets: { label: "Presets", one: "preset", many: "presets" },
} as const;

export type CollectionKey = keyof typeof COLLECTIONS;

/** Checkbox order in the backup export dialog (ticket 87). */
export const EXPORT_ORDER: readonly CollectionKey[] = [
  "launch_entries",
  "quick_actions",
  "clips",
  "products",
  "presets",
];

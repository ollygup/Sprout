/// Settings-local filter index: search before hierarchy (research 0014 rule 6).
///
/// The Settings page stays one route with four Disclosure groups; when the
/// page outgrows scanning, a local filter narrows groups to matches instead of
/// adding navigation depth. Entries are data — a future knob joins by adding
/// one entry here, never by special-casing the matcher.

export type SettingsGroupKey = "general" | "dock" | "companion" | "backup";

export const SETTINGS_GROUPS: { key: SettingsGroupKey; label: string }[] = [
  { key: "general", label: "General" },
  { key: "dock", label: "Dock" },
  { key: "companion", label: "Companion" },
  { key: "backup", label: "Backup & housekeeping" },
];

/// Knob ids per group, backing the section count badges and the resolver.
export const SETTINGS_GROUP_KNOBS: Record<SettingsGroupKey, string[]> = {
  general: ["theme", "install-dir", "autostart", "default-timeout", "log-retention", "launch-concurrency"],
  dock: ["dock-state", "dock-mode", "dock-edge", "dock-width", "dock-density", "per-monitor", "reveal-dwell", "reveal-sensitivity"],
  companion: ["companion-active", "companion-height", "companion-sites"],
  backup: ["backup", "updates"],
};

export interface SettingsSearchEntry {
  group: SettingsGroupKey;
  /** Knob id, or `group:<key>` for a whole-group match (e.g. mute, which the
   *  dock pane toolbar owns — the filter surfaces the owning group). */
  id: string;
  label: string;
  synonyms: string[];
  /** Current values as searchable text, so `dark` or `18%` land. */
  values: string[];
  description: string;
}

/// The live values the index reads. Plain data so tests can build an index
/// without rendering the page.
export interface SettingsSearchSnapshot {
  themeMode: string;
  themeLabel: string;
  installDir: string;
  autostart: string;
  timeoutMinutes: number;
  retentionDays: number;
  launchConcurrency: number;
  dockMode: string;
  dockEdge: string;
  dockState: string;
  dockWidthPct: number;
  dockDensity: string;
  revealDwellMs: number;
  revealSensitivityPx: number;
  companionActiveName: string | null;
  companionRatioPct: number;
  companionSiteCount: number;
  companionSiteNames: string[];
  companionMuted: boolean;
  updateSummary: string;
}

/// Builds the full knob + group index for one snapshot of current values.
export function buildSettingsSearchIndex(snap: SettingsSearchSnapshot): SettingsSearchEntry[] {
  const installValue = snap.installDir.trim() || "(winget default)";
  const companionValue = snap.companionActiveName ?? "Off";
  return [
    {
      group: "general",
      id: "group:general",
      label: "General",
      synonyms: ["settings", "defaults"],
      values: [],
      description: "Theme, install directory, auto-start, and run defaults.",
    },
    {
      group: "general",
      id: "theme",
      label: "Theme",
      synonyms: ["appearance", "look", "mode", "system", "light", "dark"],
      values: [snap.themeMode, snap.themeLabel],
      description: "Follows Windows or pins the app light or dark. Applies immediately.",
    },
    {
      group: "general",
      id: "install-dir",
      label: "Install directory",
      synonyms: ["location", "folder", "path", "directory", "winget default"],
      values: [installValue],
      description: "Where installs and upgrades land. Empty means the installer's default.",
    },
    {
      group: "general",
      id: "autostart",
      label: "Start with Windows",
      synonyms: ["autostart", "login", "boot", "tray", "startup", "on", "off"],
      values: [snap.autostart],
      description: "Starts Sprout with Windows, resident in the tray.",
    },
    {
      group: "general",
      id: "default-timeout",
      label: "Default timeout",
      synonyms: ["minutes", "kill", "long", "min"],
      values: [`${snap.timeoutMinutes} min`, `${snap.timeoutMinutes} minutes`],
      description: "Minutes a requirement may take before its installer is killed.",
    },
    {
      group: "general",
      id: "log-retention",
      label: "Log retention",
      synonyms: ["logs", "prune", "days", "archive", "history"],
      values: [`${snap.retentionDays} days`],
      description: "How long a finished run's raw log folder is kept.",
    },
    {
      group: "general",
      id: "launch-concurrency",
      label: "Launch concurrency",
      synonyms: ["parallel", "queue", "apps", "at once", "gentle", "snappy"],
      values: [`${snap.launchConcurrency} apps`],
      description: "How many Quick Launch apps may start at once before the rest queue.",
    },
    {
      group: "dock",
      id: "group:dock",
      label: "Dock",
      synonyms: ["quick launch", "window", "bar", "strip", "palette"],
      values: [],
      description: "How the Quick Launch window floats, docks, and reveals.",
    },
    {
      group: "dock",
      id: "dock-state",
      label: "Quick Launch window",
      synonyms: ["floating", "docked", "dock", "palette", "bar"],
      values: [snap.dockState],
      description: "Whether the Quick Launch window floats or docks to a screen edge.",
    },
    {
      group: "dock",
      id: "dock-mode",
      label: "Dock mode",
      synonyms: ["auto-hide", "autohide", "fixed", "pinned", "taskbar", "hide", "reveal"],
      values: [snap.dockMode],
      description: "Auto-hide slides the dock away when not hovered; fixed reserves the strip.",
    },
    {
      group: "dock",
      id: "dock-edge",
      label: "Default dock edge",
      synonyms: ["left", "right", "side", "screen edge"],
      values: [snap.dockEdge],
      description: "Which screen edge the dock attaches to when first docked.",
    },
    {
      group: "dock",
      id: "dock-width",
      label: "Dock width",
      synonyms: ["wide", "narrow", "size", "percent", "pixels", "px", "slider"],
      values: [`${snap.dockWidthPct}%`],
      description: "How wide the dock is, as a share of the monitor.",
    },
    {
      group: "dock",
      id: "dock-density",
      label: "List density",
      synonyms: ["compact", "default", "large", "text size", "rows", "readable"],
      values: [snap.dockDensity],
      description: "List text size in the Quick Launch window and dock.",
    },
    {
      group: "dock",
      id: "per-monitor",
      label: "Per-monitor dock",
      synonyms: ["monitor", "monitors", "display", "displays", "screen", "screens", "multi"],
      values: [],
      description: "Each display remembers its own edge, mode, and width.",
    },
    {
      group: "dock",
      id: "reveal-dwell",
      label: "Reveal delay",
      synonyms: ["dwell", "hold", "hover", "slide out", "milliseconds", "ms", "sensitivity", "snappy", "graze"],
      values: [`${snap.revealDwellMs} ms`],
      description: "Hold time at the screen edge before the hidden dock slides out.",
    },
    {
      group: "dock",
      id: "reveal-sensitivity",
      label: "Reveal sensitivity",
      synonyms: ["push", "nudge", "pixels", "px", "brushes"],
      values: [`${snap.revealSensitivityPx} px`],
      description: "Distance the cursor must push into the edge before the hold timer starts.",
    },
    {
      group: "companion",
      id: "group:companion",
      label: "Companion",
      synonyms: ["sound", "audio", "mute", "muted", "unmute", "volume", "music", "browser", "web", "site", "pane"],
      values: [companionValue],
      description: "The docked web pane: active site, height, saved sites, and audio.",
    },
    {
      group: "companion",
      id: "companion-active",
      label: "Active site",
      synonyms: ["url", "off", "pane", "website", "page"],
      values: [companionValue],
      description: "What appears while Quick Launch is docked. Off removes the pane.",
    },
    {
      group: "companion",
      id: "companion-height",
      label: "Pane height",
      synonyms: ["tall", "short", "ratio", "divider", "drag", "splitter", "percent"],
      values: [`${snap.companionRatioPct}%`],
      description: "The pane's starting height. Dragging the divider also saves.",
    },
    {
      group: "companion",
      id: "companion-sites",
      label: "Saved sites",
      synonyms: ["manage", "add", "rename", "delete", "names", "urls", "list"],
      values: snap.companionSiteNames,
      description: `${snap.companionSiteCount} site${snap.companionSiteCount === 1 ? "" : "s"} saved on this PC.`,
    },
    {
      group: "backup",
      id: "group:backup",
      label: "Backup & housekeeping",
      synonyms: ["export", "restore", "update", "updates"],
      values: [],
      description: "Whole-app backup and Sprout updates.",
    },
    {
      group: "backup",
      id: "backup",
      label: "Backup",
      synonyms: ["export", "restore", "json", "collections", "file"],
      values: [],
      description: "Writes collections into one JSON file; restoring adds what's missing.",
    },
    {
      group: "backup",
      id: "updates",
      label: "Sprout updates",
      synonyms: ["update", "upgrade", "version", "release", "github", "new build", "install"],
      values: [snap.updateSummary],
      description: "Checks GitHub releases for a newer build.",
    },
  ];
}

/// Multi-keyword match: every whitespace-separated token must appear somewhere
/// in the entry's label, synonyms, values, or description. Returns matched ids.
export function matchSettingsSearch(index: SettingsSearchEntry[], query: string): Set<string> {
  const tokens = query.toLowerCase().split(/\s+/).filter(Boolean);
  const matched = new Set<string>();
  if (tokens.length === 0) return matched;
  for (const entry of index) {
    const haystack = [entry.label, entry.description, ...entry.synonyms, ...entry.values]
      .join("\n")
      .toLowerCase();
    if (tokens.every((token) => haystack.includes(token))) matched.add(entry.id);
  }
  return matched;
}

export interface SettingsFilterResolution {
  visibleKnobIds: Set<string>;
  wholeGroups: Set<SettingsGroupKey>;
}

/// Resolves a query to visible knobs. Knob matches win: when any knob
/// matched, only those knobs show. A group match surfaces its whole group
/// only when no knob matched — for bare group-name queries and for concepts
/// no knob owns (mute lives in the dock pane toolbar, so it lands on the
/// whole Companion group instead of a knob that does not exist).
export function resolveSettingsFilter(
  index: SettingsSearchEntry[],
  query: string,
): SettingsFilterResolution {
  const matched = matchSettingsSearch(index, query);
  const visibleKnobIds = new Set(
    index
      .filter((entry) => !entry.id.startsWith("group:") && matched.has(entry.id))
      .map((entry) => entry.id),
  );
  const wholeGroups = new Set<SettingsGroupKey>();
  if (visibleKnobIds.size === 0) {
    for (const entry of index) {
      if (entry.id.startsWith("group:") && matched.has(entry.id)) {
        wholeGroups.add(entry.group);
        for (const id of SETTINGS_GROUP_KNOBS[entry.group]) visibleKnobIds.add(id);
      }
    }
  }
  return { visibleKnobIds, wholeGroups };
}

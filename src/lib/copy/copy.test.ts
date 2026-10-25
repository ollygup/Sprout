import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it } from "vitest";
import en from "./en.json";
import zh from "./zh-CN.json";
import { resolveCopy, t, tCount } from "./index";
import { setLocaleForTest } from "./locale.svelte";

// WHY: JSON imports carry literal-key types, so coverage loops read through
// a plain record view — the loader itself owns the fallback, never the test.
const EN: Record<string, string> = en as Record<string, string>;
const ZH: Record<string, string> = zh as Record<string, string>;

// Locale switches are global — every test below leaves English behind.
afterEach(() => {
  setLocaleForTest("en");
});

// Loader fallback: a partial locale reads through to English per key, and an
// unknown key reads back as itself so the UI never blanks.
describe("copy loader fallback", () => {
  it("serves English for known keys", () => {
    expect(t("settings.theme.label")).toBe(en["settings.theme.label"]);
    expect(t("presence.details")).toBe("Using Sprout");
  });

  it("reads unknown keys back as themselves, never blank", () => {
    expect(t("nope.missing.key")).toBe("nope.missing.key");
  });

  it("prefers the override but falls back to English per key", () => {
    expect(
      resolveCopy("settings.theme.label", {
        "settings.theme.label": "Tema",
      }),
    ).toBe("Tema");
    expect(resolveCopy("settings.theme.label", {})).toBe(
      en["settings.theme.label"],
    );
    expect(resolveCopy("settings.theme.label")).toBe(
      en["settings.theme.label"],
    );
  });

  it("falls back to English for keys the override does not carry", () => {
    expect(resolveCopy("dock.bezel.tooltip", {})).toBe(
      en["dock.bezel.tooltip"],
    );
  });

  it("fills count slots without losing the fallback", () => {
    expect(tCount("settings.companion-sites.hintMany", 3)).toContain("3");
    expect(tCount("missing.count.key", 3)).toBe("missing.count.key");
  });
});

describe("Simplified-Chinese file swap", () => {
  it("carries exactly the English key set — no new keys, none missing", () => {
    expect(Object.keys(ZH).sort()).toEqual(Object.keys(EN).sort());
  });

  it("gives every English key a non-empty Simplified-Chinese value", () => {
    for (const key of Object.keys(EN)) {
      expect(ZH[key], key).toBeTruthy();
      expect(ZH[key].trim(), key).not.toBe("");
    }
  });

  it("keeps every {count} slot the English source carries", () => {
    for (const [key, value] of Object.entries(EN)) {
      if (value.includes("{count}")) expect(ZH[key], key).toContain("{count}");
    }
  });

  it("serves Simplified Chinese once the locale switches, English after", () => {
    setLocaleForTest("zh-CN");
    expect(t("settings.theme.label")).toBe(ZH["settings.theme.label"]);
    expect(t("presence.details")).toBe(ZH["presence.details"]);
    expect(t("dock.bezel.tooltip")).toBe(ZH["dock.bezel.tooltip"]);
    setLocaleForTest("en");
    expect(t("settings.theme.label")).toBe(EN["settings.theme.label"]);
  });

  it("reads unknown keys back as themselves in either locale, never blank", () => {
    setLocaleForTest("zh-CN");
    expect(t("nope.missing.key")).toBe("nope.missing.key");
    setLocaleForTest("en");
    expect(t("nope.missing.key")).toBe("nope.missing.key");
  });

  it("reads through a partial override to the active locale per key", () => {
    setLocaleForTest("zh-CN");
    expect(
      resolveCopy("settings.theme.label", {
        "settings.theme.label": "主题",
      }),
    ).toBe("主题");
    expect(resolveCopy("settings.theme.hint", {})).toBe(
      ZH["settings.theme.hint"],
    );
    setLocaleForTest("en");
    expect(resolveCopy("settings.theme.hint", {})).toBe(
      EN["settings.theme.hint"],
    );
  });

  it("fills count slots in Simplified Chinese too", () => {
    setLocaleForTest("zh-CN");
    expect(tCount("settings.companion-sites.hintMany", 3)).toContain("3");
    expect(tCount("settings.companion-sites.hintMany", 3)).toContain(
      ZH["settings.companion-sites.hintMany"].replace("{count}", "3"),
    );
  });

  it("switching to zh-CN and back leaves no missing-key gaps", () => {
    for (const locale of ["zh-CN", "en", "zh-CN"] as const) {
      setLocaleForTest(locale);
      for (const key of Object.keys(EN)) {
        const rendered = t(key);
        expect(rendered, `${locale}:${key}`).toBeTruthy();
        expect(rendered.trim(), `${locale}:${key}`).not.toBe("");
        expect(rendered, `${locale}:${key}`).not.toBe(key);
      }
    }
  });

  it("keeps every {slot} the English source carries, in both locales", () => {
    const slots = (value: string) =>
      [...value.matchAll(/\{([a-z]+)\}/g)].map((m) => m[1]).sort().join(",");
    for (const [key, value] of Object.entries(EN)) {
      expect(slots(ZH[key]), key).toBe(slots(value));
    }
  });

  it("fills every {count} slot without losing the fallback", () => {
    for (const [key, value] of Object.entries(EN)) {
      if (!value.includes("{count}")) continue;
      expect(tCount(key, 3), key).toContain("3");
      expect(tCount(key, 3), key).not.toContain("{count}");
      setLocaleForTest("zh-CN");
      expect(tCount(key, 3), `zh-CN:${key}`).toContain("3");
      expect(tCount(key, 3), `zh-CN:${key}`).not.toContain("{count}");
      setLocaleForTest("en");
    }
  });
});

const SETTINGS_KEYS = [
  "settings.theme.label",
  "settings.theme.hint",
  "settings.animation.label",
  "settings.animation.hint",
  "settings.native-frame.label",
  "settings.native-frame.hint",
  "settings.install-dir.label",
  "settings.install-dir.hint",
  "settings.autostart.label",
  "settings.autostart.hint",
  "settings.default-timeout.label",
  "settings.default-timeout.hint",
  "settings.log-retention.label",
  "settings.log-retention.hint",
  "settings.launch-concurrency.label",
  "settings.launch-concurrency.hint",
  "settings.dock-state.label",
  "settings.dock-state.hint",
  "settings.dock-mode.label",
  "settings.dock-mode.hint",
  "settings.dock-edge.label",
  "settings.dock-edge.hint",
  "settings.dock-width.label",
  "settings.dock-width.hint",
  "settings.dock-width.multiHint",
  "settings.bezel-y.hint",
  "settings.dock-density.label",
  "settings.dock-density.hint",
  "settings.per-monitor.label",
  "settings.per-monitor.hint",
  "settings.reveal-dwell.label",
  "settings.reveal-dwell.hint",
  "settings.reveal-sensitivity.label",
  "settings.reveal-sensitivity.hint",
  "settings.companion-active.label",
  "settings.companion-active.hint",
  "settings.companion-height.label",
  "settings.companion-height.hint",
  "settings.companion-sites.label",
  "settings.companion-sites.hintOne",
  "settings.companion-sites.hintMany",
  "settings.backup.label",
  "settings.backup.hint",
  "settings.updates.label",
  "settings.updates.hint",
  "settings.ai-provider.label",
  "settings.ai-provider.hint",
  "settings.ai-provider.hintManaged",
  "settings.ai-provider.hintCloud",
  "settings.ai-managed.label",
  "settings.ai-managed.hint",
  "settings.ai-active.label",
  "settings.ai-active.hint",
  "settings.ai-available.label",
  "settings.ai-available.hint",
  "settings.ai-unavailable.label",
  "settings.ai-unavailable.hint",
  "settings.ai-endpoint.label",
  "settings.ai-endpoint.hint",
  "settings.ai-model.label",
  "settings.ai-model.hint",
  "settings.ai-downloads.label",
  "settings.ai-downloads.hint",
];

const DOCK_KEYS = [
  "dock.open.tooltip",
  "dock.edgeLeft.tooltip",
  "dock.edgeRight.tooltip",
  "dock.undock.tooltip",
  "dock.close.tooltip",
  "dock.seam.tooltip",
  "dock.bezel.tooltip",
  "dock.bezel.peekHint",
];

const PRESENCE_KEYS = [
  "presence.details",
  "presence.state.launching",
  "presence.state.products",
  "presence.state.presets",
  "presence.state.plan",
  "presence.state.history",
  "presence.state.logs",
  "presence.state.settings",
  "presence.state.clips",
  "presence.state.quick-actions",
];

describe("copy key coverage over V1 scope", () => {
  it("carries every Settings knob label and hint", () => {
    for (const key of SETTINGS_KEYS) {
      expect(EN[key], key).toBeTruthy();
    }
  });

  it("carries every dock tooltip including the bezel pair for future 217", () => {
    for (const key of DOCK_KEYS) {
      expect(EN[key], key).toBeTruthy();
    }
  });

  it("carries presence details plus the full 9-state allowlist wording", () => {
    for (const key of PRESENCE_KEYS) {
      expect(EN[key], key).toBeTruthy();
    }
    expect(en["presence.details"]).toBe("Using Sprout");
    expect(en["presence.state.launching"]).toBe("Launching apps");
    expect(en["presence.state.presets"]).toBe("Composing presets");
  });

  it("keeps every hint at most 140 characters", () => {
    for (const [key, value] of Object.entries(en)) {
      if (/hint/i.test(key)) expect(value.length, key).toBeLessThanOrEqual(140);
    }
  });

  it("holds ticket 33's bans: no Sprout-never phrasing, no loading rotator", () => {
    for (const [key, value] of Object.entries(en)) {
      expect(value, key).not.toMatch(/Sprout never/);
      expect(value, key).not.toMatch(/Loading/);
    }
  });
});

const SETTINGS_SOURCE = readFileSync(
  new URL("../../routes/settings/+page.svelte", import.meta.url),
  "utf8",
);
const QUICK_SOURCE = readFileSync(
  new URL("../../routes/quick-launch-window/+page.svelte", import.meta.url),
  "utf8",
);
const CHROME_SOURCE = readFileSync(
  new URL("../windowChrome.ts", import.meta.url),
  "utf8",
);
const LAYOUT_SOURCE = readFileSync(
  new URL("../../routes/+layout.svelte", import.meta.url),
  "utf8",
);
const LOADER_SOURCE = readFileSync(
  new URL("./index.ts", import.meta.url),
  "utf8",
);

describe("copy wiring across V1 scope", () => {
  it("records the voice rules on the loader", () => {
    expect(LOADER_SOURCE).toMatch(/noun titles/);
    expect(LOADER_SOURCE).toMatch(/Discord-tone/);
    expect(LOADER_SOURCE).toMatch(/140/);
    expect(LOADER_SOURCE).toMatch(/zh-CN/);
  });

  it("wires Settings knob labels and hints through the dictionary", () => {
    expect(SETTINGS_SOURCE).toMatch(/from "\$lib\/copy"/);
    expect(SETTINGS_SOURCE).toContain('t("settings.theme.label")');
    expect(SETTINGS_SOURCE).toContain('t("settings.theme.hint")');
    expect(SETTINGS_SOURCE).toContain('t("settings.dock-width.hint")');
    expect(SETTINGS_SOURCE).toContain('t("settings.ai-provider.hint")');
    expect(SETTINGS_SOURCE).toContain('t("dock.seam.tooltip")');
    expect(SETTINGS_SOURCE).not.toContain(
      "Follows Windows, or pins one look",
    );
    expect(SETTINGS_SOURCE).not.toContain("Wider fits longer names");
  });

  it("reuses the one seam key instead of duplicating its text", () => {
    expect(SETTINGS_SOURCE).not.toContain(
      'const SEAM_REASON = "Borders another display',
    );
    expect(QUICK_SOURCE).not.toContain(
      'const SEAM_REASON = "Borders another display',
    );
    expect(en["dock.seam.tooltip"]).toBe(
      "Borders another display — cursor can't stop there",
    );
  });

  it("wires dock chrome tooltips through the dictionary, row content untouched", () => {
    expect(QUICK_SOURCE).toMatch(/from "\$lib\/copy"/);
    expect(QUICK_SOURCE).toContain('t("dock.open.tooltip")');
    expect(QUICK_SOURCE).toContain('t("dock.edgeLeft.tooltip")');
    expect(QUICK_SOURCE).toContain('t("dock.edgeRight.tooltip")');
    expect(QUICK_SOURCE).toContain('t("dock.undock.tooltip")');
    expect(QUICK_SOURCE).toContain('t("dock.close.tooltip")');
    expect(QUICK_SOURCE).not.toContain('title="Open Sprout"');
    expect(QUICK_SOURCE).not.toContain('"Dock to the left edge"');
    expect(QUICK_SOURCE).not.toContain('"Dock to the right edge"');
    expect(QUICK_SOURCE).not.toContain('"Undock — float again"');
    expect(QUICK_SOURCE).not.toContain('label="Close window"');
  });

  it("reads section labels from the dictionary, keeping the same nine routes", () => {
    expect(CHROME_SOURCE).toContain('"/": "presence.state.launching"');
    expect(CHROME_SOURCE).toContain('"/presets": "presence.state.presets"');
    expect(CHROME_SOURCE).toContain(
      '"/quick-actions": "presence.state.quick-actions"',
    );
    expect(CHROME_SOURCE).toMatch(/sectionLabelForRoute[\s\S]*t\(/);
    expect(CHROME_SOURCE).toMatch(/englishCopy\["presence\.state\.launching"\]/);
    expect(CHROME_SOURCE).not.toContain('"/": "Launching apps"');
  });

  it("reports presence from the dictionary while keeping the shared route map", () => {
    expect(LAYOUT_SOURCE).toContain('englishCopy["presence.details"]');
    expect(LAYOUT_SOURCE).not.toContain('t("presence.details")');
    expect(LAYOUT_SOURCE).toMatch(/SECTION_LABEL_BY_ROUTE\[page\.route\.id/);
  });
});

// Full-app scope: every user-facing menu, button, placeholder, dropdown,
// page title, description, dialog, empty-state, and notice string resolves
// through the dictionary — translation stays a file swap.
const FULL_SCOPE_SECTIONS = [
  "nav.",
  "chrome.",
  "error.",
  "common.",
  "dialog.",
  "menu.",
  "groups.",
  "drag.",
  "features.",
  "select.",
  "dockfilter.",
  "collection.",
  "policy.",
  "action.",
  "status.",
  "outcome.",
  "ai.",
  "packet.",
  "details.",
  "qdetails.",
  "test.",
  "launch.",
  "products.",
  "presets.",
  "run.",
  "plan.",
  "history.",
  "logs.",
  "clips.",
  "actions.",
  "companion.",
  "quickwindow.",
  "managed.",
  "files.",
  "prereq.",
  "clipform.",
  "commandform.",
  "presetform.",
  "productform.",
  "actionform.",
  "settings.",
  "search.",
  "update.",
];

describe("copy key coverage over full-app scope", () => {
  it("carries every surface section beside the V1 dock/presence/settings trio", () => {
    for (const prefix of FULL_SCOPE_SECTIONS) {
      expect(
        Object.keys(EN).filter((key) => key.startsWith(prefix)).length,
        prefix,
      ).toBeGreaterThan(0);
    }
  });

  it("keys every rail section plus the window chrome verbs", () => {
    for (const key of [
      "nav.launch",
      "nav.actions",
      "nav.clips",
      "nav.products",
      "nav.presets",
      "nav.plan",
      "nav.history",
      "nav.logs",
      "nav.settings",
      "nav.companion",
      "chrome.minimize",
      "chrome.restore",
      "chrome.maximize",
      "common.close",
      "chrome.skipLink",
    ]) {
      expect(EN[key], key).toBeTruthy();
      expect(ZH[key], key).toBeTruthy();
    }
  });

  it("keys every shared menu verb, dialog verb, and dock-filter choice", () => {
    for (const key of [
      "menu.moveUp",
      "menu.moveDown",
      "menu.moveToGroup",
      "menu.ungrouped",
      "menu.newGroup",
      "menu.hideFromDock",
      "menu.installNow",
      "menu.planWithThis",
      "menu.fork",
      "menu.export",
      "menu.download",
      "menu.downloadAll",
      "menu.noAssignment",
      "menu.currentDesktop",
      "menu.virtualDesktop",
      "menu.newDesktop",
      "common.add",
      "common.edit",
      "common.remove",
      "common.save",
      "common.saveChanges",
      "common.cancel",
      "common.close",
      "common.discard",
      "common.retry",
      "common.refresh",
      "common.showAll",
      "common.clearSearch",
      "common.clearFilter",
      "common.clearFilters",
      "common.on",
      "common.off",
      "dockfilter.all",
      "dockfilter.shown",
      "dockfilter.label",
      "dockfilter.reset",
      "select.chooseOption",
    ]) {
      expect(EN[key], key).toBeTruthy();
      expect(ZH[key], key).toBeTruthy();
    }
  });

  it("keys every dropdown option label including theme, dock, and AI routes", () => {
    for (const key of [
      "settings.opt.theme.system",
      "settings.opt.theme.light",
      "settings.opt.theme.dark",
      "settings.opt.mode.auto",
      "settings.opt.mode.fixed",
      "settings.opt.mode.bezel",
      "settings.opt.edge.left",
      "settings.opt.edge.right",
      "settings.opt.state.floating",
      "settings.opt.state.docked",
      "settings.opt.density.compact",
      "settings.opt.density.default",
      "settings.opt.density.large",
      "settings.opt.frame.modern",
      "settings.opt.frame.native",
      "ai.existing",
      "ai.managedShort",
      "ai.cloudLater",
      "policy.latest",
      "policy.pinned",
      "policy.present",
      "policy.latestHint",
      "policy.pinnedHint",
      "policy.presentHint",
    ]) {
      expect(EN[key], key).toBeTruthy();
      expect(ZH[key], key).toBeTruthy();
    }
  });

  it("keys every search placeholder and filter label", () => {
    for (const key of [
      "launch.filterPh",
      "launch.filterLabel",
      "launch.searchAppsPh",
      "launch.searchAppsLabel",
      "products.filterPh",
      "clips.searchPh",
      "clips.searchLabel",
      "actions.searchPh",
      "actions.searchLabel",
      "settings.filterPh",
      "settings.filterLabel",
      "common.searchLibrary",
      "common.clearSearch",
    ]) {
      expect(EN[key], key).toBeTruthy();
      expect(ZH[key], key).toBeTruthy();
    }
  });

  it("keys every page subtitle, empty state, and remove dialog", () => {
    for (const key of [
      "launch.trayHint",
      "launch.emptyTitle",
      "launch.noFilterTitle",
      "launch.removeTitle",
      "products.hintRight",
      "products.noProducts",
      "products.removeTitle",
      "presets.hintRight",
      "presets.noPresets",
      "presets.removeTitle",
      "plan.subPick",
      "plan.noPresets",
      "history.subtitle",
      "history.noRuns",
      "logs.subtitle",
      "clips.subBody",
      "clips.noClips",
      "clips.deleteTitle",
      "actions.subBody",
      "actions.noActions",
      "actions.removeTitle",
      "companion.subtitle",
      "companion.noSites",
      "companion.removeTitle",
      "quickwindow.emptyLaunch",
      "quickwindow.emptyActions",
    ]) {
      expect(EN[key], key).toBeTruthy();
      expect(ZH[key], key).toBeTruthy();
    }
  });

  it("keys every run, plan-action, and status label behind badges and toasts", () => {
    for (const key of [
      "action.install",
      "action.upgrade",
      "action.unmanaged",
      "status.installed",
      "status.upgraded",
      "status.alreadyOk",
      "status.newer",
      "status.unmanaged",
      "status.failed",
      "status.timedOut",
      "outcome.ok",
      "outcome.notes",
      "outcome.cancelled",
      "outcome.failed",
      "run.cancel",
      "run.cancelling",
      "run.inProgress",
      "run.okTitle",
      "run.notesTitle",
      "run.failedTitle",
      "run.cancelledTitle",
      "run.rebootRequired",
      "run.noResults",
      "run.again",
    ]) {
      expect(EN[key], key).toBeTruthy();
      expect(ZH[key], key).toBeTruthy();
    }
  });

  it("keeps every hint at most 140 characters across the full dictionary", () => {
    for (const [key, value] of Object.entries(en)) {
      if (/hint/i.test(key)) expect(value.length, key).toBeLessThanOrEqual(140);
    }
  });

  it("holds the bans across the full dictionary: no Sprout-never phrasing, no loading rotator", () => {
    for (const [key, value] of Object.entries(en)) {
      expect(value, key).not.toMatch(/Sprout never/);
      expect(value, key).not.toMatch(/Loading/);
    }
    for (const [key, value] of Object.entries(zh)) {
      expect(value, key).not.toMatch(/Loading/);
    }
  });
});

const WIRED_SOURCES: [string, string][] = [
  ["chrome", "UnifiedHeader.svelte"],
  ["rail", "NavRail.svelte"],
  ["search input", "SearchInput.svelte"],
  ["select", "Select.svelte"],
  ["dock filter", "DockVisibilityFilter.svelte"],
  ["features", "PageFeaturesButton.svelte"],
  ["dialog", "Dialog.svelte"],
  ["confirm", "ConfirmDialog.svelte"],
  ["group naming", "GroupNameDialog.svelte"],
  ["test result", "TestResult.svelte"],
  ["banner", "RunBanner.svelte"],
  ["run control", "QuickActionRunControl.svelte"],
  ["packet card", "PacketCard.svelte"],
  ["preset packet", "PresetPacket.svelte"],
  ["product packet", "ProductPacket.svelte"],
  ["product details", "ProductDetailsDialog.svelte"],
  ["action details", "QuickActionDetailsDialog.svelte"],
  ["clip form", "ClipFormDialog.svelte"],
  ["command form", "CommandFormDialog.svelte"],
  ["preset form", "PresetFormDialog.svelte"],
  ["product form", "ProductFormDialog.svelte"],
  ["action form", "QuickActionFormDialog.svelte"],
];

function readSource(relative: string): string {
  return readFileSync(new URL(relative, import.meta.url), "utf8");
}

describe("copy wiring across full-app scope", () => {
  it("imports the dictionary in every component that renders chrome copy", () => {
    for (const [label, file] of WIRED_SOURCES) {
      expect(readSource(`../components/${file}`), label).toMatch(
        /from "\$lib\/copy"/,
      );
    }
  });

  it("wires window chrome, rail, and layout through the dictionary", () => {
    const header = readSource("../components/UnifiedHeader.svelte");
    expect(header).toContain('t("chrome.minimize")');
    expect(header).toContain('t("chrome.maximize")');
    expect(header).toContain('t("chrome.barLabel")');
    expect(header).not.toContain('label="Minimize"');
    const rail = readSource("../components/NavRail.svelte");
    expect(rail).toContain('t("nav.plan")');
    expect(rail).toContain('t("nav.settings")');
    expect(rail).toContain('t("update.installRestart")');
    expect(rail).not.toContain('label: "Plan"');
    expect(LAYOUT_SOURCE).toContain('t("chrome.skipLink")');
  });

  it("wires menus, filters, and run controls without duplicating verbs", () => {
    const groups = readSource("../collectionGroups.svelte.ts");
    expect(groups).toContain('t("menu.ungrouped")');
    expect(groups).toContain('t("menu.newGroup")');
    expect(groups).toContain('t("groups.flashOff")');
    expect(groups).not.toContain('label: "Move to group"');
    expect(groups).not.toContain(': "Groups off — groups and assignments');
    // The "Move to group" parent row is built by the pages that own the row
    // menu, reusing the shared manager's children.
    for (const page of [
      "../../routes/+page.svelte",
      "../../routes/clips/+page.svelte",
      "../../routes/quick-actions/+page.svelte",
    ]) {
      expect(readSource(page), page).toContain('t("menu.moveToGroup")');
    }
    const filter = readSource("../components/DockVisibilityFilter.svelte");
    expect(filter).toContain('t("dockfilter.shown")');
    expect(filter).toContain('t("dockfilter.reset")');
    expect(filter).not.toContain('label: "All"');
    const run = readSource("../components/QuickActionRunControl.svelte");
    expect(run).toContain('t("action.runName")');
    expect(run).toContain('t("action.stop")');
    const banner = readSource("../components/RunBanner.svelte");
    expect(banner).toContain('t("run.cancel")');
    expect(banner).toContain('t("run.inProgress")');
    expect(banner).not.toContain('"Cancel run"');
  });

  it("wires dialogs, packets, and forms through shared keys", () => {
    expect(readSource("../components/ConfirmDialog.svelte")).toContain(
      't("common.cancel")',
    );
    expect(readSource("../components/GroupNameDialog.svelte")).toContain(
      't("groups.createTitle")',
    );
    expect(readSource("../components/PresetPacket.svelte")).toContain(
      't("packet.importedTitle")',
    );
    expect(readSource("../components/ProductPacket.svelte")).toContain(
      't("packet.customInstall")',
    );
    expect(readSource("../components/QuickActionDetailsDialog.svelte")).toContain(
      't("qdetails.showCommand")',
    );
    expect(readSource("../components/ClipFormDialog.svelte")).toContain(
      't("clipform.addBtn")',
    );
    expect(readSource("../components/CommandFormDialog.svelte")).toContain(
      't("commandform.showWindow")',
    );
    expect(readSource("../components/PresetFormDialog.svelte")).toContain(
      't("presetform.addApp")',
    );
    expect(readSource("../components/ProductFormDialog.svelte")).toContain(
      't("productform.addVar")',
    );
    expect(readSource("../components/QuickActionFormDialog.svelte")).toContain(
      't("actionform.genDraft")',
    );
    expect(readSource("../components/TestResult.svelte")).toContain(
      't("test.timeoutBody")',
    );
  });

  it("resolves display labels through functions, never frozen maps", () => {
    const types = readSource("../types.ts");
    expect(types).toContain("policyLabelText");
    expect(types).toContain("actionLabelText");
    expect(types).toContain("runStatusLabelText");
    expect(types).toContain("runOutcomeLabelText");
    expect(types).toContain("aiProviderLabelText");
    expect(types).not.toContain("policyLabel:");
    expect(types).not.toContain("actionLabel:");
    expect(types).not.toContain("runStatusLabel:");
    expect(types).not.toContain("runOutcomeLabel:");
    expect(types).not.toContain("aiProviderLabel:");
  });

  it("wires every page header, placeholder, and remove dialog", () => {
    const pages: [string, string[]][] = [
      ["../../routes/+page.svelte", ['t("nav.launch")', 't("launch.filterPh")', 't("launch.removeTitle")']],
      ["../../routes/products/+page.svelte", ['t("nav.products")', 't("products.filterPh")', 't("products.removeTitle")']],
      ["../../routes/presets/+page.svelte", ['t("nav.presets")', 't("presets.compose")', 't("presets.removeTitle")']],
      ["../../routes/plan/+page.svelte", ['t("nav.plan")', 't("plan.stepPick")', 't("dialog.stopRun.title")']],
      ["../../routes/history/+page.svelte", ['t("nav.history")', 't("history.openInPlan")', 't("history.noRuns")']],
      ["../../routes/logs/+page.svelte", ['t("nav.logs")', 't("logs.openFolder")', 't("logs.emptyRuns")']],
      ["../../routes/clips/+page.svelte", ['t("nav.clips")', 't("clips.searchPh")', 't("clips.deleteTitle")']],
      ["../../routes/quick-actions/+page.svelte", ['t("nav.actions")', 't("actions.searchPh")', 't("actions.warnTitle")']],
      ["../../routes/companion/+page.svelte", ['t("nav.companion")', 't("companion.addSite")', 't("companion.removeTitle")']],
      [
        "../../routes/quick-launch-window/+page.svelte",
        ['t("quickwindow.startAll")', 't("quickwindow.tabsLabel")', 't("quickwindow.bezelPosition")'],
      ],
      ["../../routes/settings/+page.svelte", ['t("settings.group.general")', 't("settings.filterPh")', 't("settings.dirtyTitle")']],
    ];
    for (const [file, keys] of pages) {
      const source = readSource(file);
      for (const key of keys) expect(source, `${file}:${key}`).toContain(key);
    }
  });

  it("leaves the banned rotator and backend protocol strings alone", () => {
    const quickWindow = readSource(
      "../../routes/quick-launch-window/+page.svelte",
    );
    expect(quickWindow).toContain("Loading…");
    expect(quickWindow).toContain('"webview not found"');
    const plan = readSource("../../routes/plan/+page.svelte");
    expect(plan).toContain('"ignored the requested directory"');
    expect(plan).not.toContain("will install");
  });
});

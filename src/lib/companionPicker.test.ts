import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import type { CompanionSite } from "./types";
import {
  companionPickerLabel,
  createCompanionSiteSwitchQueue,
} from "./companion";

const ROUTE_SOURCE = readFileSync(
  new URL("../routes/quick-launch-window/+page.svelte", import.meta.url),
  "utf8",
);

function site(url: string, name = "", ua: "mobile" | "desktop" = "mobile"): CompanionSite {
  return { url, name, ua };
}

function deferred() {
  let resolve!: () => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function settle() {
  await Promise.resolve();
  await Promise.resolve();
}

describe("Companion saved-site picker", () => {
  it("shows both identity and address for named sites", () => {
    expect(companionPickerLabel(site("https://example.com/work", "Work"))).toBe(
      "Work — https://example.com/work",
    );
    expect(companionPickerLabel(site("https://example.com"))).toBe(
      "https://example.com",
    );
  });

  it("shows the user-configured name only in picker rows", () => {
    // Names are unique at authoring (duplicates refused); blank names fall
    // back to the address via companionDisplayName. The trigger tooltip keeps
    // the full name + address for long/similar entries.
    const menuAt = ROUTE_SOURCE.indexOf("const companionSiteMenu");
    expect(menuAt).toBeGreaterThan(-1);
    expect(ROUTE_SOURCE.slice(menuAt, menuAt + 800)).toContain(
      "label: companionDisplayName(site)",
    );
  });

  it("keeps the single-site label plain and discloses multiple sites semantically", () => {
    expect(ROUTE_SOURCE).toContain("{#if companionHasSitePicker}");
    expect(ROUTE_SOURCE).toContain('aria-haspopup="menu"');
    expect(ROUTE_SOURCE).toContain("aria-expanded={companionSiteMenuOpen}");
    expect(ROUTE_SOURCE).toContain("<ContextMenu ctx={companionSiteMenu}");
    expect(ROUTE_SOURCE).toMatch(
      /\{:else\}\s*<span class="qlw__companion-url"[^>]*>/,
    );
  });

  it("keeps the external action separate and leaves the site visible under the picker", () => {
    const selector = ROUTE_SOURCE.indexOf('aria-haspopup="menu"');
    const external = ROUTE_SOURCE.indexOf('label={companionOpeningExternal');
    expect(selector).toBeGreaterThan(-1);
    expect(external).toBeGreaterThan(selector);
    // The details dialog still yields the native child, but the site menu
    // opens upward over web content so the page stays visible underneath.
    expect(ROUTE_SOURCE).toContain("const overlayOpen = detailsAction !== null;");
    expect(ROUTE_SOURCE).toContain('placement: "above"');
    expect(ROUTE_SOURCE).not.toContain(
      "detailsAction !== null || companionSiteMenuOpen",
    );
  });

  it("contains long labels and announces pending switches without claiming success", () => {
    // Rows show the name only; the trigger tooltip keeps the full
    // name + address for long/similar entries.
    expect(ROUTE_SOURCE).toContain("companionPickerLabel(activeCompanionSite)");
    expect(ROUTE_SOURCE).toContain("overflow-wrap: anywhere");
    expect(ROUTE_SOURCE).toContain('aria-live="polite"');
    expect(ROUTE_SOURCE).toContain('aria-busy={companionSwitchingTo !== null}');
    expect(ROUTE_SOURCE).toContain('{companionSwitchingTo ? "Switching…"');
  });

  it("serializes rapid requests and applies only the newest successful site", async () => {
    const first = deferred();
    const second = deferred();
    const persist = vi
      .fn<(url: string) => Promise<void>>()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const pending: Array<CompanionSite | null> = [];
    const applied: CompanionSite[] = [];
    const failed: Array<{ site: CompanionSite; error: unknown }> = [];
    const request = createCompanionSiteSwitchQueue({
      persist,
      onPending: (value) => pending.push(value),
      onApplied: (value) => applied.push(value),
      onFailure: (value, error) => failed.push({ site: value, error }),
    });
    const work = site("https://work.example", "Work", "desktop");
    const music = site("https://music.example", "Music");

    request(work);
    request(work);
    request(music);
    expect(persist).toHaveBeenCalledTimes(1);
    first.resolve();
    await settle();
    expect(persist).toHaveBeenNthCalledWith(2, music.url);
    expect(applied).toEqual([]);

    second.resolve();
    await settle();
    expect(applied).toEqual([music]);
    expect(failed).toEqual([]);
    expect(pending).toEqual([work, music, null]);
  });

  it("ignores an obsolete failure but reports a failure for the latest request", async () => {
    const first = deferred();
    const second = deferred();
    const persist = vi
      .fn<(url: string) => Promise<void>>()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const applied: CompanionSite[] = [];
    const failed: Array<{ site: CompanionSite; error: unknown }> = [];
    const request = createCompanionSiteSwitchQueue({
      persist,
      onPending: () => {},
      onApplied: (value) => applied.push(value),
      onFailure: (value, error) => failed.push({ site: value, error }),
    });
    const firstSite = site("https://first.example");
    const lastSite = site("https://last.example");

    request(firstSite);
    request(lastSite);
    first.reject(new Error("obsolete"));
    await settle();
    expect(failed).toEqual([]);

    const finalError = new Error("save refused");
    second.reject(finalError);
    await settle();
    expect(applied).toEqual([]);
    expect(failed).toEqual([{ site: lastSite, error: finalError }]);
  });
});

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { trailApplyVisit } from "./companion";
import {
  clampCompanionUserZoom,
  companionEffectiveZoom,
  companionWebviewBounds,
  companionZoomForWidth,
  formatCompanionZoomPct,
  stepCompanionUserZoom,
} from "./companionPane";

const ROUTE_SOURCE = readFileSync(
  new URL("../routes/quick-launch-window/+page.svelte", import.meta.url),
  "utf8",
);
const API_SOURCE = readFileSync(new URL("./api.ts", import.meta.url), "utf8");
const THEME_SOURCE = readFileSync(new URL("./theme.svelte.ts", import.meta.url), "utf8");
const SETTINGS_SOURCE = readFileSync(
  new URL("../routes/settings/+page.svelte", import.meta.url),
  "utf8",
);
const CARGO_SOURCE = readFileSync(
  new URL("../../src-tauri/Cargo.toml", import.meta.url),
  "utf8",
);
const QUICK_WINDOW_SOURCE = readFileSync(
  new URL("../../src-tauri/src/quick_window.rs", import.meta.url),
  "utf8",
);
const APPBAR_SOURCE = readFileSync(
  new URL("../../src-tauri/src/appbar.rs", import.meta.url),
  "utf8",
);
const LIB_SOURCE = readFileSync(
  new URL("../../src-tauri/src/lib.rs", import.meta.url),
  "utf8",
);
const SETTINGS_RS_SOURCE = readFileSync(
  new URL("../../src-tauri/src/settings.rs", import.meta.url),
  "utf8",
);
const AUDIO_RS_SOURCE = readFileSync(
  new URL("../../src-tauri/src/companion_audio.rs", import.meta.url),
  "utf8",
);
const HISTORY_RS_SOURCE = readFileSync(
  new URL("../../src-tauri/src/companion_history.rs", import.meta.url),
  "utf8",
);
const TYPES_SOURCE = readFileSync(new URL("./types.ts", import.meta.url), "utf8");

describe("Companion native WebView contract", () => {
  it("uses the content frame's logical bounds without covering the toolbar", () => {
    expect(
      companionWebviewBounds({ left: 1, top: 642, width: 338, height: 356 }),
    ).toEqual({ x: 1, y: 642, width: 338, height: 356 });
  });

  it("positions the native WebView over the content frame, below its toolbar", () => {
    expect(ROUTE_SOURCE).toMatch(/bind:this=\{companionFrameWrapEl\}/);
    expect(ROUTE_SOURCE).toMatch(/companionWebviewBounds\(companionFrameWrapEl/);
  });

  it("keeps narrow responsive sites near a 320 CSS-pixel layout width", () => {
    expect(companionZoomForWidth(340)).toBe(1);
    expect(companionZoomForWidth(288)).toBeCloseTo(0.9);
    expect(companionZoomForWidth(224)).toBe(0.7);
    expect(companionZoomForWidth(180)).toBe(0.7);
  });

  it("hands Open externally to the operating system", () => {
    expect(ROUTE_SOURCE).not.toContain('window.open(companionUrl, "_blank")');
    expect(ROUTE_SOURCE).toContain("openCompanionExternal(companionUrl)");
  });

  it("uses a Chromium-compatible mobile identity for WebView2", () => {
    expect(API_SOURCE).toContain("Chrome/150.0.0.0 Mobile Safari/537.36");
    expect(API_SOURCE).not.toContain("CPU iPhone OS");
  });

  it("offers a per-site Desktop Edge identity with the Windows token", () => {
    // Desktop-only sites load with the Edge variant; Win11 differs only via
    // Client Hints so `Windows NT 10.0` covers current releases.
    expect(API_SOURCE).toContain("COMPANION_DESKTOP_UA");
    expect(API_SOURCE).toContain("Windows NT 10.0");
    expect(API_SOURCE).toContain("Edg/150.0.0.0");
    expect(ROUTE_SOURCE).toContain("companionUserAgentForUrl");
    expect(ROUTE_SOURCE).toContain("COMPANION_DESKTOP_UA");
  });

  it("holds a failed URL until the user retries instead of flickering", () => {
    expect(ROUTE_SOURCE).toContain(
      "if (companionWebviewFailed && companionFailedUrl === companionUrl) return;",
    );
    expect(ROUTE_SOURCE).toContain("companionFailedUrl = null;");
  });

  it("serializes URL replacement and ignores stale WebView callbacks", () => {
    expect(ROUTE_SOURCE).toContain("while (companionSyncPending)");
    expect(ROUTE_SOURCE).toContain("const targetUrl = companionUrl;");
    expect(ROUTE_SOURCE).toContain("if (companionWebview !== wv) return;");
  });

  it("remeasures the native child when the content frame changes size", () => {
    expect(ROUTE_SOURCE).toContain("new ResizeObserver(");
    expect(ROUTE_SOURCE).toContain("observer.observe(frame)");
    // The observer also records the content width for the zoom readout —
    // measuring never resizes the frame itself.
    expect(ROUTE_SOURCE).toContain("companionFrameWidth = frame.getBoundingClientRect().width");
  });

  it("offers a keyboard alternative for resizing the pane", () => {
    expect(ROUTE_SOURCE).toContain("onkeydown={onCompanionSplitterKeyDown}");
    expect(ROUTE_SOURCE).toContain('tabindex="0"');
    expect(ROUTE_SOURCE).toContain(
      "aria-valuenow={Math.round(companionRatio * 100)}",
    );
  });

  it("keeps Companion alive for both dock visibility modes", () => {
    expect(ROUTE_SOURCE).toContain(
      "const companionVisible = $derived(dock.docked && hasCompanionUrl(companionUrl));",
    );
    expect(ROUTE_SOURCE).not.toMatch(/companionVisible[^\n]*(auto-hide|fixed)/);
  });

  it("manages the multi-WebView host through its native parent window", () => {
    expect(QUICK_WINDOW_SOURCE).toContain(
      "app.get_window(QUICK_LAUNCH_WINDOW)",
    );
    expect(QUICK_WINDOW_SOURCE).not.toContain("get_webview_window(");
  });

  it("notifies the multi-WebView Quick Launch window through its native parent", () => {
    // Tauri 2.11.5's get_webview_window rejects a native window once it hosts
    // a differently labeled child WebView — with Companion up it yields None,
    // so every quick-launch-changed / launch-run-done emit through it is
    // silently dropped and Settings saves never reach the dock (stale active
    // URL, stale blocked banner, stuck Start state).
    expect(LIB_SOURCE).not.toContain(
      "get_webview_window(quick_window::QUICK_LAUNCH_WINDOW)",
    );
    // The fan-out emit resolves its label in a loop variable — pin that too,
    // so a revert there cannot silently re-break the dock while Companion is up.
    expect(LIB_SOURCE).not.toContain("get_webview_window(label)");
    // All Quick Launch resolutions share quick_window's native-Window seam —
    // one lookup, every emitter and probe fixed together.
    expect(LIB_SOURCE).toContain("quick_window::quick_launch_window(&app)");
  });

  it("leaves a converged auto-hide dock alone on dock-irrelevant saves", () => {
    // A Companion-URL save must not kick the settled driver or re-probe the
    // shell: the spurious settle re-logs reservation grants and can surface a
    // refusal the change had nothing to do with. Real transitions converge
    // through apply_settings, which registers on its own paths.
    expect(QUICK_WINDOW_SOURCE).toContain("needs_reestablish(&current.edge");
  });

  it("names the auto-hide slot owner when registration is refused", () => {
    // "Another bar owns this edge" is unactionable without knowing whether
    // the holder is us, a ghost, or a foreign bar — the refusal log must say.
    expect(APPBAR_SOURCE).toContain("describe_autohide_owner");
  });

  it("tells the truth when auto-hide registration is refused", () => {
    // The driver slides the strip regardless of the shell registration
    // (appbar.rs, CONTEXT Quick Launch dock) — the banner must never claim
    // the strip is pinned while hiding works.
    expect(ROUTE_SOURCE).not.toContain(
      "The strip stays pinned until that edge frees up",
    );
    expect(ROUTE_SOURCE).toContain("Hiding still works");
  });

  it("stacks the blocked banner so it stays usable at the dock's real width", () => {
    // 340 physical px at 150% scaling is ~226 CSS px: a single flex row lets
    // the recovery action hold its width while the reason crushes to one
    // character per line. The banner keeps its copy and order but stacks the
    // action below an icon + reason row.
    expect(ROUTE_SOURCE).toContain("qlw__blocked-top");
    expect(ROUTE_SOURCE).toMatch(
      /\.qlw__blocked\s*\{[^}]*flex-direction:\s*column/,
    );
  });

  it("forwards Sprout's concrete theme to child WebViews", () => {
    // Pins push their concrete native theme; system mode passes null (follow
    // the OS) so the forced value never poisons the matchMedia read the
    // system resolution depends on — see themeSystem.test.ts for the loop.
    expect(THEME_SOURCE).toContain("getCurrentWindow().setTheme(native)");
    expect(THEME_SOURCE).toContain('mode === "system" ? null : mode');
  });

  it("reconciles the live dock after the complete Settings batch", () => {
    const perDisplaySave = SETTINGS_SOURCE.indexOf("await setDisplayDockMode");
    const finalReconcile = SETTINGS_SOURCE.indexOf("await reconcileQuickLaunchSettings()");
    expect(perDisplaySave).toBeGreaterThan(-1);
    expect(finalReconcile).toBeGreaterThan(perDisplaySave);
  });

  it("enables Tauri's child-WebView API and keeps a separate profile", () => {
    expect(CARGO_SOURCE).toMatch(/features = \[[^\]]*"unstable"/);
    expect(ROUTE_SOURCE).toContain('dataDirectory: "companion"');
  });
});

describe("Companion audio: toolbar mute + playing indicator", () => {
  it("mutes through WebView2's mute-only API and reads playback from it", () => {
    expect(AUDIO_RS_SOURCE).toContain("SetIsMuted");
    expect(AUDIO_RS_SOURCE).toContain("IsMuted");
    expect(AUDIO_RS_SOURCE).toContain("IsDocumentPlayingAudio");
    expect(AUDIO_RS_SOURCE).not.toContain("SetVolume");
  });

  it("persists one global mute, default unmuted", () => {
    expect(SETTINGS_RS_SOURCE).toContain("settings.companion_muted");
    expect(SETTINGS_RS_SOURCE).toContain("DEFAULT_COMPANION_MUTED");
    expect(SETTINGS_RS_SOURCE).toMatch(/DEFAULT_COMPANION_MUTED:\s*bool\s*=\s*false/);
    expect(LIB_SOURCE).toContain("fn get_companion_audio_state");
    expect(LIB_SOURCE).toContain("fn set_companion_muted");
    expect(API_SOURCE).toContain("get_companion_audio_state");
    expect(API_SOURCE).toContain("set_companion_muted");
  });

  it("heals a fresh WebView toward the persisted mute on read and creation", () => {
    expect(ROUTE_SOURCE).toContain("getCompanionAudioState()");
    expect(ROUTE_SOURCE).toContain("setCompanionMuted(!wasMuted)");
    expect(AUDIO_RS_SOURCE).toContain("apply_muted(app, persisted)");
  });

  it("keeps the toggle keyboard-accessible with pressed state and the indicator tooltip-only", () => {
    expect(ROUTE_SOURCE).toContain("aria-pressed={companionMuted}");
    expect(ROUTE_SOURCE).toContain("Mute companion audio");
    expect(ROUTE_SOURCE).toContain("Unmute companion audio");
    expect(ROUTE_SOURCE).toContain("qlw__companion-playing");
    expect(ROUTE_SOURCE).toContain("Playing audio");
    expect(ROUTE_SOURCE).toContain('role="img"');
  });

  it("gates every audio chrome on the docked pane — floating and no-URL states gain none", () => {
    const barAt = ROUTE_SOURCE.indexOf("qlw__companion-bar");
    const toggleAt = ROUTE_SOURCE.indexOf("onclick={toggleCompanionMute}");
    const indicatorAt = ROUTE_SOURCE.indexOf("qlw__companion-playing");
    expect(barAt).toBeGreaterThan(-1);
    // The toggle and the indicator both live inside the docked pane's own
    // toolbar, which itself renders only while companionVisible holds.
    expect(toggleAt).toBeGreaterThan(barAt);
    expect(indicatorAt).toBeGreaterThan(barAt);
    expect(ROUTE_SOURCE).toContain("{#if companionVisible}");
  });

  it("links loud/soft straight to the OS mixer instead of describing the way in words", () => {
    // The moment-of-use shortcut supersedes the earlier Settings paragraph:
    // no audio help text lives in Settings anymore.
    expect(SETTINGS_SOURCE).not.toContain("Volume Mixer");
    expect(SETTINGS_SOURCE).not.toContain("volume slider");
    expect(LIB_SOURCE).toContain("fn open_volume_mixer");
    expect(LIB_SOURCE).toContain("ms-settings:apps-volume");
    expect(API_SOURCE).toContain("open_volume_mixer");
    expect(ROUTE_SOURCE).toContain("onclick={openMixer}");
    expect(ROUTE_SOURCE).toContain("Open volume mixer");
    expect(ROUTE_SOURCE).toContain('icon="sliders"');
    // The shortcut lives inside the docked pane's own toolbar, like the rest
    // of the audio chrome — floating and no-URL states gain nothing.
    const barAt = ROUTE_SOURCE.indexOf("qlw__companion-bar");
    expect(ROUTE_SOURCE.indexOf("onclick={openMixer}")).toBeGreaterThan(barAt);
  });
});

describe("Companion height resolve never fails silently (ticket 143)", () => {
  it("retries a bounded number of times when the monitor is absent", () => {
    // Right after a dock toggle the backend state can lag the window — one
    // shot at the per-monitor read turns a transient into a permanent
    // fallback. Retry is bounded so a genuinely monitorless dock still ends.
    expect(ROUTE_SOURCE).toContain("RESOLVE_ATTEMPTS");
    expect(ROUTE_SOURCE).toContain("RESOLVE_RETRY_MS");
    expect(ROUTE_SOURCE).toContain("setTimeout");
  });

  it("says so when it genuinely cannot resolve the screen", () => {
    expect(ROUTE_SOURCE).toContain(
      "Couldn't read the Companion height for this screen",
    );
  });

  it("says so when a drag persist fails instead of looking applied", () => {
    expect(ROUTE_SOURCE).toContain("Couldn't save the Companion height");
  });
});

describe("Companion user zoom per site (ticket 162)", () => {
  it("clamps explicit zoom into 50–200%, reading anything else as auto", () => {
    expect(clampCompanionUserZoom(1)).toBe(1);
    expect(clampCompanionUserZoom(0.1)).toBe(0.5);
    expect(clampCompanionUserZoom(5)).toBe(2);
    expect(clampCompanionUserZoom(null)).toBeNull();
    expect(clampCompanionUserZoom(undefined)).toBeNull();
    expect(clampCompanionUserZoom(Number.NaN)).toBeNull();
  });

  it("falls back to the automatic width zoom while unset", () => {
    expect(companionEffectiveZoom(0.9, null)).toBe(0.9);
    expect(companionEffectiveZoom(0.9, 1.5)).toBe(1.5);
  });

  it("steps ten points at a time within 50–200%", () => {
    expect(stepCompanionUserZoom(1, 1)).toBeCloseTo(1.1);
    expect(stepCompanionUserZoom(1, -1)).toBeCloseTo(0.9);
    expect(stepCompanionUserZoom(2, 1)).toBe(2);
    expect(stepCompanionUserZoom(0.5, -1)).toBe(0.5);
  });

  it("renders whole percents", () => {
    expect(formatCompanionZoomPct(1)).toBe("100%");
    expect(formatCompanionZoomPct(0.9)).toBe("90%");
  });

  it("puts the zoom control in the dock bar without touching the bar order", () => {
    // Builds over 161's stabilized order: zoom sits after Reload, before the
    // URL; mute/mixer/external order is untouched. Anchored on the bar
    // markup (onclick wiring + row span), not on script definitions or CSS.
    const barAt = ROUTE_SOURCE.indexOf('<div class="qlw__companion-bar"');
    const reloadAt = ROUTE_SOURCE.indexOf("onclick={() => void companionReload()}");
    const zoomAt = ROUTE_SOURCE.indexOf("onclick={() => void companionZoomStep");
    const urlAt = ROUTE_SOURCE.indexOf('<span class="qlw__companion-url"');
    expect(barAt).toBeGreaterThan(-1);
    expect(reloadAt).toBeGreaterThan(barAt);
    expect(zoomAt).toBeGreaterThan(reloadAt);
    expect(urlAt).toBeGreaterThan(zoomAt);
    expect(ROUTE_SOURCE).toContain("qlw__companion-zoom-pct");
    expect(ROUTE_SOURCE).toContain("Zoom out companion");
    expect(ROUTE_SOURCE).toContain("Zoom in companion");
  });

  it("keeps the single-row bar intact at the 340px floor (ticket 170 follow-on)", () => {
    // The bar stays one row: fixed 30px controls never crush (only the site
    // trigger squeezes, through its ellipsis), and no second row wrapper
    // exists. The chevron signals expanded state by rotation (Disclosure
    // precedent), collapsing under the global reduced-motion rule.
    expect(ROUTE_SOURCE).not.toContain("qlw__companion-site-row");
    expect(ROUTE_SOURCE).not.toContain("qlw__companion-actions");
    expect(ROUTE_SOURCE).toContain(".qlw__companion-bar :global(.icon-btn)");
    expect(ROUTE_SOURCE).toContain("flex: none;");
    expect(ROUTE_SOURCE).toContain(
      '.qlw__companion-site-trigger[aria-expanded="true"]',
    );
    expect(ROUTE_SOURCE).toContain("rotate(180deg)");
  });

  it("left-aligns the site picker to the full trigger width", () => {
    // The shared ContextMenu still right-aligns by default (⋯ row menus);
    // the Companion selector opts into start alignment + anchor width so the
    // popup reads as a select dropdown, not a right-edge menu.
    expect(ROUTE_SOURCE).toContain('align: "start"');
    expect(ROUTE_SOURCE).toContain("matchAnchorWidth: true");
  });

  it("moves infrequent controls behind the ⋯ menu only while overflowing", () => {
    // Priority+ as a last resort (0004:1): the ⋯ trigger (shared IconButton
    // + shared ContextMenu, currentTarget-anchored) exists only at stage > 0;
    // zoom + mixer hide at stage 1, mute at stage 2 — trigger, Reload and
    // Open externally never hide. The overflow menu reuses the same handlers,
    // never duplicates behavior.
    expect(ROUTE_SOURCE).toContain('icon="dots"');
    expect(ROUTE_SOURCE).toContain("More Companion actions");
    expect(ROUTE_SOURCE).toContain("data-ctx-more");
    expect(ROUTE_SOURCE).toContain("companionOverflowStage < 1");
    expect(ROUTE_SOURCE).toContain("companionOverflowStage < 2");
    expect(ROUTE_SOURCE).toContain("companionOverflowStage > 0");
    expect(ROUTE_SOURCE).toContain("fitCompanionBar");
    expect(ROUTE_SOURCE).toContain("scrollWidth");
    expect(ROUTE_SOURCE).toContain("ResizeObserver");
    const menuAt = ROUTE_SOURCE.indexOf("const companionMoreMenu");
    expect(menuAt).toBeGreaterThan(-1);
    const menuBody = ROUTE_SOURCE.slice(menuAt, menuAt + 2400);
    expect(menuBody).toContain("companionZoomStep");
    expect(menuBody).toContain("companionZoomReset");
    expect(menuBody).toContain("openMixer");
    expect(menuBody).toContain("toggleCompanionMute");
    expect(menuBody).not.toContain("companionReload");
    expect(menuBody).not.toContain("companionOpenExternal");
    expect(menuBody).not.toContain("chooseCompanionSite");
  });

  it("keeps ⋯ labels short with a reset icon", () => {
    const menuAt = ROUTE_SOURCE.indexOf("const companionMoreMenu");
    const menuBody = ROUTE_SOURCE.slice(menuAt, menuAt + 2400);
    expect(menuBody).toContain('label: "Zoom out"');
    expect(menuBody).toContain('label: "Zoom in"');
    expect(menuBody).toContain('label: "Reset zoom"');
    expect(menuBody).toContain('icon: "refresh"');
    expect(menuBody).not.toContain("Zoom out companion");
  });

  it("keeps only one companion menu open at a time", () => {
    const siteToggleAt = ROUTE_SOURCE.indexOf("function toggleCompanionSiteMenu");
    expect(siteToggleAt).toBeGreaterThan(-1);
    expect(ROUTE_SOURCE.slice(siteToggleAt, siteToggleAt + 400)).toContain(
      "companionMoreMenuOpen = false",
    );
    const moreToggleAt = ROUTE_SOURCE.indexOf("function toggleCompanionMoreMenu");
    expect(moreToggleAt).toBeGreaterThan(-1);
    expect(ROUTE_SOURCE.slice(moreToggleAt, moreToggleAt + 500)).toContain(
      "companionSiteMenuOpen = false",
    );
  });

  it("persists zoom per site and never moves the height splitter", () => {
    expect(ROUTE_SOURCE).toContain("persistCompanionUserZoom");
    expect(ROUTE_SOURCE).toContain("setCompanionUrlList");
    expect(ROUTE_SOURCE).toContain("companionStoredZoomForUrl");
    // The zoom path writes only the site list's zoom; the splitter path
    // writes only the ratio — neither function touches the other's state.
    const persistAt = ROUTE_SOURCE.indexOf("async function persistCompanionUserZoom");
    expect(persistAt).toBeGreaterThan(-1);
    const persistBody = ROUTE_SOURCE.slice(persistAt, persistAt + 1500);
    expect(persistBody).not.toContain("companionRatio");
    expect(persistBody).not.toContain("setCompanionHeightRatio");
  });
});

describe("Companion unborn window stays quiet", () => {
  it("skips bounds syncs while the child is still registering", () => {
    // setPosition/setSize/setZoom on a handle between `new Webview()` and its
    // created event throw WebviewNotFound on every pass until registration
    // lands (slow on cold WebView2 profile init) — the pass must return
    // before touching the child, not attempt and log.
    const boundsAt = ROUTE_SOURCE.indexOf("Existing webview: update bounds live");
    expect(boundsAt).toBeGreaterThan(-1);
    const boundsSource = ROUTE_SOURCE.slice(boundsAt, boundsAt + 1200);
    expect(boundsSource).toContain("if (!companionWebviewBorn) return;");
    expect(boundsSource).toContain("setPosition");
  });

  it("fails loud when registration never lands", () => {
    // The quiet passes above stay silent by design, so a birth that never
    // completes must expire loudly instead of hanging forever with no pane
    // and no word anywhere.
    expect(ROUTE_SOURCE).toContain("COMPANION_BORN_TIMEOUT_MS");
    expect(ROUTE_SOURCE).toContain("never finished loading");
  });
});

describe("Companion session trail behind Back/Forward", () => {
  const A = "https://a.example/";
  const B = "https://b.example";
  const C = "https://c.example/path";

  it("records fresh visits by dropping the forward trail", () => {
    expect(trailApplyVisit([], -1, A, false)).toEqual({ history: [A], index: 0 });
    expect(trailApplyVisit([A], 0, B, false)).toEqual({ history: [A, B], index: 1 });
    // A fresh visit from mid-trail truncates the forward entries first.
    expect(trailApplyVisit([A, B, C], 0, B, false)).toEqual({
      history: [A, B],
      index: 1,
    });
  });

  it("moves the marker on history steps without rewriting the trail", () => {
    expect(trailApplyVisit([A, B], 1, A, true)).toEqual({ history: [A, B], index: 0 });
    expect(trailApplyVisit([A, B], 0, B, true)).toEqual({ history: [A, B], index: 1 });
    // A revisit lands on its latest visit, never an earlier one.
    expect(trailApplyVisit([A, B, A], 2, A, true).index).toBe(2);
    expect(trailApplyVisit([A, B, A], 0, B, true)).toEqual({
      history: [A, B, A],
      index: 1,
    });
  });

  it("falls back to a fresh visit when the step target left the trail", () => {
    expect(trailApplyVisit([A], 0, C, true)).toEqual({ history: [A, C], index: 1 });
  });

  it("compares by URL key, so case and slash drift never fork the trail", () => {
    expect(trailApplyVisit(["https://a.example"], 0, "https://A.EXAMPLE/", true)).toEqual({
      history: ["https://a.example"],
      index: 0,
    });
  });

  it("renders Back/Forward always, disabled honestly at the trail ends", () => {
    // Never hidden-then-jumping: no conditional mount around either button,
    // so enabling never reflows the row (research 0004 rules 1 and 5).
    expect(ROUTE_SOURCE).not.toContain("{#if companionCanGoBack}");
    expect(ROUTE_SOURCE).not.toContain("{#if companionCanGoForward}");
    expect(ROUTE_SOURCE).toContain("disabled={!companionCanGoBack");
    expect(ROUTE_SOURCE).toContain("disabled={!companionCanGoForward");
    // The learned affordance keeps its icons, labels and browser order —
    // Back, Forward, then Reload.
    const barAt = ROUTE_SOURCE.indexOf('<div class="qlw__companion-bar"');
    const backAt = ROUTE_SOURCE.indexOf('label="Back"');
    const forwardAt = ROUTE_SOURCE.indexOf('label="Forward"');
    const reloadAt = ROUTE_SOURCE.indexOf("onclick={() => void companionReload()}");
    expect(barAt).toBeGreaterThan(-1);
    expect(backAt).toBeGreaterThan(barAt);
    expect(forwardAt).toBeGreaterThan(backAt);
    expect(reloadAt).toBeGreaterThan(forwardAt);
    expect(ROUTE_SOURCE).toContain('icon="chevron-left"');
    expect(ROUTE_SOURCE).toContain('icon="chevron-right"');
  });

  it("derives the disabled states from the live native child (preview: trail marker)", () => {
    // Native path prefers the live child state so in-page link traversals are
    // observed; the preview iframe keeps the trail marker.
    expect(ROUTE_SOURCE).toContain("isTauri ? companionNativeBack : companionHistoryIndex > 0,");
    expect(ROUTE_SOURCE).toContain("companionNativeForward");
    expect(ROUTE_SOURCE).toContain("companionHistoryIndex < companionHistory.length - 1,");
    // Mid-switch and mid-step the buttons wait instead of firing stale.
    expect(ROUTE_SOURCE).toContain("companionSwitchingTo !== null || companionHistoryBusy}");
    expect(ROUTE_SOURCE).toContain("let companionHistoryBusy = $state(false);");
    expect(ROUTE_SOURCE).toContain("async function refreshCompanionHistory()");
    expect(ROUTE_SOURCE).toContain("getCompanionHistoryState()");
  });

  it("drives native steps through the live child history, never a second child", () => {
    const stepAt = ROUTE_SOURCE.indexOf("function stepCompanionHistory");
    expect(stepAt).toBeGreaterThan(-1);
    const stepBody = ROUTE_SOURCE.slice(stepAt, stepAt + 1800);
    // Native branch delegates to the Rust history owner; the preview branch
    // keeps the iframe fallback — never cited as native.
    expect(stepBody).toContain("stepCompanionHistoryNative(direction)");
    expect(stepBody).toContain("companionFrameEl.src = targetUrl");
    const nativeAt = ROUTE_SOURCE.indexOf("async function stepCompanionHistoryNative");
    expect(nativeAt).toBeGreaterThan(-1);
    const nativeBody = ROUTE_SOURCE.slice(nativeAt, nativeAt + 1400);
    expect(nativeBody).toContain("nativeCompanionGoBack()");
    expect(nativeBody).toContain("nativeCompanionGoForward()");
    expect(nativeBody).not.toContain("new Webview(");
    expect(nativeBody).not.toContain("queueCompanionSiteSwitch");
    // One creation seam in the whole page: the existing child lifetime.
    expect(ROUTE_SOURCE.match(/new Webview\(/g)?.length).toBe(1);
  });

  it("says so when a native step fails instead of navigating nowhere", () => {
    const nativeAt = ROUTE_SOURCE.indexOf("async function stepCompanionHistoryNative");
    expect(nativeAt).toBeGreaterThan(-1);
    const nativeBody = ROUTE_SOURCE.slice(nativeAt, nativeAt + 1400);
    expect(nativeBody).toContain("error = String(e);");
  });

  it("records switch arrivals on the trail and clears a failed step marker", () => {
    expect(ROUTE_SOURCE).toContain("trailApplyVisit(");
    const applyAt = ROUTE_SOURCE.indexOf("function applyCompanionSite");
    expect(applyAt).toBeGreaterThan(-1);
    expect(ROUTE_SOURCE.slice(applyAt, applyAt + 1400)).toContain(
      "companionPendingTrailKey = null;",
    );
    const failureAt = ROUTE_SOURCE.indexOf("onFailure: (site, switchError)");
    expect(failureAt).toBeGreaterThan(-1);
    expect(ROUTE_SOURCE.slice(failureAt, failureAt + 500)).toContain(
      "companionPendingTrailKey = null;",
    );
  });

  it("keeps reload and retry on the saved address in every state", () => {
    const retryAt = ROUTE_SOURCE.indexOf("async function companionRetry");
    const reloadAt = ROUTE_SOURCE.indexOf("async function companionReload");
    expect(retryAt).toBeGreaterThan(-1);
    expect(reloadAt).toBeGreaterThan(retryAt);
    // Reload delegates to the single retry path — one recreate, no refresh of
    // a current page.
    expect(ROUTE_SOURCE.slice(reloadAt, reloadAt + 400)).toContain(
      "await companionRetry();",
    );
    // Retry closes the child and re-syncs; the re-sync recreates at the saved
    // address. Neither path navigates from history or a shown page.
    const retryBody = ROUTE_SOURCE.slice(retryAt, reloadAt);
    expect(retryBody).toContain("await syncCompanionWebview();");
    expect(retryBody).not.toContain("companionHistory");
    expect(retryBody).not.toContain("companionFrameEl");
    expect(ROUTE_SOURCE).toContain("const targetUrl = companionUrl;");
    // Back-then-reload returns to saved: the recreated child reports fresh
    // (disabled) history after landing.
    expect(retryBody).toContain("void refreshCompanionHistory();");
  });

  it("keeps the active-site reselect a no-op and Open externally saved-address", () => {
    const chooseAt = ROUTE_SOURCE.indexOf("function chooseCompanionSite");
    expect(chooseAt).toBeGreaterThan(-1);
    const chooseBody = ROUTE_SOURCE.slice(chooseAt, chooseAt + 800);
    expect(chooseBody).toContain("return;");
    expect(chooseBody).toContain("queueCompanionSiteSwitch(site)");
    // A fresh pick clears a stale Back/Forward failure line at request time.
    expect(chooseBody).toContain("Couldn't go back");
    expect(ROUTE_SOURCE).toContain("openCompanionExternal(companionUrl)");
    expect(ROUTE_SOURCE).not.toContain("openCompanionExternal(companionHistory");
  });

  it("keeps Try again plus Open externally on the failure pane", () => {
    const failureAt = ROUTE_SOURCE.indexOf("qlw__companion-failure");
    expect(failureAt).toBeGreaterThan(-1);
    const failureBody = ROUTE_SOURCE.slice(failureAt, failureAt + 1400);
    expect(failureBody).toContain("() => void companionRetry()");
    expect(failureBody).toContain("() => void companionOpenExternal()");
  });
});

describe("Companion native history over the live child (ticket 177)", () => {
  it("owns traversal in one Rust module with no second invocation site", () => {
    expect(HISTORY_RS_SOURCE).toContain("GoBack");
    expect(HISTORY_RS_SOURCE).toContain("GoForward");
    expect(HISTORY_RS_SOURCE).toContain("CanGoBack");
    expect(HISTORY_RS_SOURCE).toContain("CanGoForward");
    expect(HISTORY_RS_SOURCE).toContain("add_HistoryChanged");
    expect(HISTORY_RS_SOURCE).toContain("add_NavigationCompleted");
    expect(HISTORY_RS_SOURCE).toContain("companion-history-changed");
    // The child label is reused from the audio owner, never redeclared.
    expect(HISTORY_RS_SOURCE).toContain("companion_audio::COMPANION_WEBVIEW_LABEL");
    expect(HISTORY_RS_SOURCE).not.toContain('"companion"');
    // No popup/new-window interception, no relaxed isolation here.
    expect(HISTORY_RS_SOURCE).not.toContain("NewWindowRequested");
    expect(HISTORY_RS_SOURCE).not.toContain("SetHandled");
  });

  it("registers the history commands beside the audio commands", () => {
    expect(LIB_SOURCE).toContain("fn get_companion_history_state");
    expect(LIB_SOURCE).toContain("fn companion_go_back");
    expect(LIB_SOURCE).toContain("fn companion_go_forward");
    expect(LIB_SOURCE).toContain("fn ensure_companion_history_hook");
    expect(LIB_SOURCE).toContain("get_companion_history_state,");
    expect(LIB_SOURCE).toContain("companion_go_back,");
    expect(LIB_SOURCE).toContain("companion_go_forward,");
    expect(LIB_SOURCE).toContain("ensure_companion_history_hook,");
  });

  it("exposes the history seam to the dock toolbar", () => {
    expect(TYPES_SOURCE).toContain("interface CompanionHistoryState");
    expect(TYPES_SOURCE).toContain("can_go_back: boolean;");
    expect(TYPES_SOURCE).toContain("can_go_forward: boolean;");
    expect(API_SOURCE).toContain("getCompanionHistoryState");
    expect(API_SOURCE).toContain("companionGoBack");
    expect(API_SOURCE).toContain("companionGoForward");
    expect(API_SOURCE).toContain("ensureCompanionHistoryHook");
    expect(API_SOURCE).toContain("COMPANION_HISTORY_CHANGED_EVENT");
    expect(API_SOURCE).toContain("get_companion_history_state");
    expect(API_SOURCE).toContain("companion_go_back");
    expect(API_SOURCE).toContain("companion_go_forward");
    expect(API_SOURCE).toContain("ensure_companion_history_hook");
  });

  it("listens for native navigations and hooks the live child on creation", () => {
    expect(ROUTE_SOURCE).toContain("COMPANION_HISTORY_CHANGED_EVENT");
    expect(ROUTE_SOURCE).toContain("ensureCompanionHistoryHook()");
    expect(ROUTE_SOURCE).toContain("refreshCompanionHistory()");
    // The picker stays the sole site-switching surface.
    const chooseAt = ROUTE_SOURCE.indexOf("function chooseCompanionSite");
    expect(chooseAt).toBeGreaterThan(-1);
    expect(ROUTE_SOURCE.slice(chooseAt, chooseAt + 800)).toContain(
      "queueCompanionSiteSwitch(site)",
    );
  });

  it("keeps the saved-address picker as the sole switch surface", () => {
    // Back/Forward never duplicate a switch the picker already offers: the
    // native step body contains no saved-site lookup or switch queue.
    const nativeAt = ROUTE_SOURCE.indexOf("async function stepCompanionHistoryNative");
    expect(nativeAt).toBeGreaterThan(-1);
    const nativeBody = ROUTE_SOURCE.slice(nativeAt, nativeAt + 1400);
    expect(nativeBody).not.toContain("companionUrlList.find");
    expect(nativeBody).not.toContain("queueCompanionSiteSwitch");
  });
});

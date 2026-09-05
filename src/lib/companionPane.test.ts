import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  companionWebviewBounds,
  companionZoomForWidth,
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
    expect(API_SOURCE).toContain("Chrome/131.0.0.0 Mobile Safari/537.36");
    expect(API_SOURCE).not.toContain("CPU iPhone OS");
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
    expect(ROUTE_SOURCE).toContain("new ResizeObserver(() => void syncCompanionWebview())");
    expect(ROUTE_SOURCE).toContain("observer.observe(frame)");
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
    expect(THEME_SOURCE).toContain("getCurrentWindow().setTheme(applied)");
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

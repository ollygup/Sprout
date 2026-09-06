import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const DIALOG_SOURCE = readFileSync(
  new URL("./components/QuickActionDetailsDialog.svelte", import.meta.url),
  "utf8",
);
const ROUTE_SOURCE = readFileSync(
  new URL("../routes/quick-launch-window/+page.svelte", import.meta.url),
  "utf8",
);
const QUICK_LAUNCH_CAPABILITY = JSON.parse(
  readFileSync(
    new URL("../../src-tauri/capabilities/quick-launch.json", import.meta.url),
    "utf8",
  ),
) as { permissions: string[] };
const DEFAULT_CAPABILITY = JSON.parse(
  readFileSync(
    new URL("../../src-tauri/capabilities/default.json", import.meta.url),
    "utf8",
  ),
) as { permissions: string[] };

describe("Quick Action details note-first contract (ticket 141)", () => {
  it("renders the full note above the command scent", () => {
    const noteAt = DIALOG_SOURCE.indexOf("note-block--first");
    const cmdAt = DIALOG_SOURCE.indexOf("cmd-block");
    expect(noteAt).toBeGreaterThan(-1);
    expect(cmdAt).toBeGreaterThan(noteAt);
    // The old command-first definition list is gone — no mono wall on top.
    expect(DIALOG_SOURCE).not.toMatch(/<dt>Command<\/dt>/);
  });

  it("collapses the command to a 3-line scent with Show-command and Copy", () => {
    expect(DIALOG_SOURCE).toContain("-webkit-line-clamp: 3");
    expect(DIALOG_SOURCE).toContain("cmd-scent--clamped");
    expect(DIALOG_SOURCE).toContain("Show command");
    expect(DIALOG_SOURCE).toContain("aria-expanded={showFullCommand}");
    expect(DIALOG_SOURCE).toContain('aria-controls="qa-details-command"');
    expect(DIALOG_SOURCE).toContain('id="qa-details-command"');
    expect(DIALOG_SOURCE).toContain("navigator.clipboard.writeText");
    expect(DIALOG_SOURCE).toContain("Copied");
    expect(DIALOG_SOURCE).toContain('aria-live="polite"');
  });

  it("points note-less commands at the main app for the full text", () => {
    expect(DIALOG_SOURCE).toContain("Find the full command in the main app.");
  });

  it("shows the toggle and hint only when the command actually overflows", () => {
    // A short command already shows fully — a Show-command button that flips
    // to Hide with no visible change (and a main-app hint pointing at text
    // already on screen) is noise. Overflow is measured post-paint, never in
    // render, following the run-control measure-on-mount precedent.
    expect(DIALOG_SOURCE).toContain("commandOverflows");
    expect(DIALOG_SOURCE).toContain("scrollHeight");
    expect(DIALOG_SOURCE).toContain("clientHeight");
    expect(DIALOG_SOURCE).toContain("{#if showFullCommand || commandOverflows}");
    expect(DIALOG_SOURCE).toContain(
      "{#if !hasRenderedNote && commandOverflows && !showFullCommand}",
    );
  });

  it("keeps Run/Stop/Stopping ownership untouched", () => {
    expect(DIALOG_SOURCE).toContain("QuickActionRunControl");
    expect(DIALOG_SOURCE).toContain("onrun={() => onrun(action)}");
    expect(DIALOG_SOURCE).toContain("onstop={() => onstop(action)}");
  });

  it("adds no motion or ad-hoc styling to the dialog", () => {
    expect(DIALOG_SOURCE).not.toContain("transition: all");
    expect(DIALOG_SOURCE).not.toContain("@keyframes");
    expect(DIALOG_SOURCE).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
  });
});

describe("Companion yield while the details dialog is open (ticket 141)", () => {
  it("hides the native child while open and re-syncs after", () => {
    expect(ROUTE_SOURCE).toContain("detailsAction !== null");
    expect(ROUTE_SOURCE).toContain("webview.hide()");
    expect(ROUTE_SOURCE).toContain("await webview.show()");
    expect(ROUTE_SOURCE).toContain("await syncCompanionWebview()");
  });

  it("starts a freshly created child yielded when the dialog is already open", () => {
    // The re-assertion lives in the created callback: a synchronous hide
    // right after construction races backend registration (and throws
    // WebviewNotFound noise while unborn), so the birth moment owns the yield.
    const createdAt = ROUTE_SOURCE.indexOf('wv.once("tauri://created"');
    expect(createdAt).toBeGreaterThan(-1);
    const errorAt = ROUTE_SOURCE.indexOf(
      'wv.once("tauri://error"',
      createdAt,
    );
    expect(errorAt).toBeGreaterThan(createdAt);
    expect(ROUTE_SOURCE.slice(createdAt, errorAt)).toContain(
      "if (detailsAction !== null)",
    );
  });

  it("never hides a registering child synchronously after construction", () => {
    // Hiding between `new Webview()` and its created event can only lose to
    // backend creation — and every such call logs WebviewNotFound until
    // registration lands. The created callback above owns the first yield.
    const assignedAt = ROUTE_SOURCE.indexOf("companionWebview = wv;");
    expect(assignedAt).toBeGreaterThan(-1);
    expect(ROUTE_SOURCE.slice(assignedAt, assignedAt + 800)).not.toContain(
      "wv.hide()",
    );
  });

  it("declares the hide/show permissions the yield invokes", () => {    // hide()/show() invoke plugin:webview|webview_hide/_show — Tauri denies
    // any command outside the window's capability, and the denial only
    // surfaces as a console error, so the pane silently keeps covering the
    // dialog. Pinning the permissions here is the seam that catches that.
    expect(QUICK_LAUNCH_CAPABILITY.permissions).toContain(
      "core:webview:allow-webview-hide",
    );
    expect(QUICK_LAUNCH_CAPABILITY.permissions).toContain(
      "core:webview:allow-webview-show",
    );
  });

  it("declares the set-theme permission the native theme sync invokes", () => {
    // syncNativeWindowTheme calls Window.setTheme — the same denial class as
    // the hide/show denial above, and the denial only reaches console.error.
    // The theme module runs in both windows (the layout starts it, the dock
    // restores it), so both capabilities must grant it.
    expect(QUICK_LAUNCH_CAPABILITY.permissions).toContain(
      "core:window:allow-set-theme",
    );
    expect(DEFAULT_CAPABILITY.permissions).toContain(
      "core:window:allow-set-theme",
    );
  });

  it("surfaces a refused yield in the error line instead of only the console", () => {
    expect(ROUTE_SOURCE).toContain("Couldn't hide the companion pane");
    expect(ROUTE_SOURCE).toContain("Couldn't restore the companion pane");
  });

  it("keeps a registering child quiet while the dialog yields", () => {
    // hide()/show() on a child still registering throw WebviewNotFound until
    // its created event lands — the yield must skip those calls while unborn
    // instead of attempting and logging; the created callback re-asserts the
    // yield at birth, so the skip loses nothing.
    const yieldAt = ROUTE_SOURCE.indexOf("always sits above the Companion pane");
    expect(yieldAt).toBeGreaterThan(-1);
    expect(ROUTE_SOURCE.slice(yieldAt)).toContain(
      "if (!companionWebviewBorn) return;",
    );
  });

  it("hides the Loading text while the dialog holds the yield", () => {
    // Hiding the native child exposes the placeholder beneath it — a
    // "Loading…" line behind the dialog reads as a stuck load. The wrapper
    // stays (it reserves the native bounds); only the text gates on the
    // dialog being closed (research 0004 rule 2: no misleading chrome).
    const placeholderAt = ROUTE_SOURCE.indexOf("qlw__companion-placeholder-text");
    expect(placeholderAt).toBeGreaterThan(-1);
    const before = ROUTE_SOURCE.slice(
      Math.max(0, placeholderAt - 300),
      placeholderAt,
    );
    expect(before).toContain("{#if detailsAction === null}");
  });

  it("resolves the live child by label instead of trusting the cached handle", () => {
    // A close that races its null-out leaves the cached handle pointing at a
    // dead label — restoring through it fails with "webview not found" while
    // the pane stays bricked. The yield must resolve the live child first.
    const yieldAt = ROUTE_SOURCE.indexOf("always sits above the Companion pane");
    expect(yieldAt).toBeGreaterThan(-1);
    const yieldSource = ROUTE_SOURCE.slice(yieldAt);
    expect(ROUTE_SOURCE).toContain("async function liveCompanionChild");
    expect(ROUTE_SOURCE).toContain('Webview.getByLabel("companion")');
    expect(yieldSource).toContain("liveCompanionChild()");
  });

  it("tracks child birth so a dead handle recreates instead of throwing forever", () => {
    // Bounds syncs on a stale handle throw on every pass while needsCreate
    // stays false — the pane never comes back. A birth flag distinguishes a
    // child that died (drop + recreate) from one still registering.
    expect(ROUTE_SOURCE).toContain("companionWebviewBorn");
  });
});

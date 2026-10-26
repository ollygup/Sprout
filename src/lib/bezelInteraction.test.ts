import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const PAGE_SOURCE = readFileSync(
  new URL("../routes/quick-launch-window/+page.svelte", import.meta.url),
  "utf8",
);
const API_SOURCE = readFileSync(new URL("./api.ts", import.meta.url), "utf8");

/** The body of one script function, sliced by name — hover paths must show
 *  their exact writes, so a future edit that opens from hover fails loudly. */
function fnBody(name: string): string {
  const start = PAGE_SOURCE.indexOf(`function ${name}(`);
  expect(start, `${name} exists`).toBeGreaterThanOrEqual(0);
  const markers = [
    "\n  async function ",
    "\n  function ",
    "\n  const ",
    "\n  let ",
    "\n  $effect(",
  ];
  const ends = markers
    .map((m) => PAGE_SOURCE.indexOf(m, start + 1))
    .filter((i) => i > start);
  return PAGE_SOURCE.slice(start, Math.min(...ends));
}

describe("bezel peek/open/close interaction (spec 214)", () => {
  it("peeks on hover only — hover can never open the panel", () => {
    const peek = fnBody("bezelPeek");
    expect(peek).toContain('placeBezelRect("peek", false, true)');
    expect(peek).toContain("bezelPeeking = true");
    expect(peek).not.toContain("bezelOpen = ");
    expect(peek).not.toContain("setBezelOpen(");
    const unpeek = fnBody("bezelUnpeek");
    expect(unpeek).toContain('placeBezelRect("collapsed", true, true)');
    expect(unpeek).toContain("bezelPeeking = false");
    expect(unpeek).not.toContain("bezelOpen = ");
    expect(unpeek).not.toContain("setBezelOpen(");
    // Wired to hover AND focus anticipation only — focus peeks, never opens.
    expect(PAGE_SOURCE).toContain("onmouseenter={() => void bezelPeek()}");
    expect(PAGE_SOURCE).toContain("onmouseleave={() => void bezelUnpeek()}");
    expect(PAGE_SOURCE).toContain("onfocus={() => void bezelPeek()}");
    expect(PAGE_SOURCE).toContain("onblur={() => void bezelUnpeek()}");
    // The hover trigger lives on the window root, never the tab button: a
    // wall press clamps onto the window border outside the button, which
    // would read as a leave and flicker (research 0011 study B).
    const rootStart = PAGE_SOURCE.indexOf('class="qlw"');
    const rootOpen = PAGE_SOURCE.slice(rootStart, PAGE_SOURCE.indexOf("\n>", rootStart));
    expect(rootOpen).toContain("onmouseenter={() => void bezelPeek()}");
    expect(rootOpen).toContain("onmouseleave={() => void bezelUnpeek()}");
    const tabStart = PAGE_SOURCE.indexOf('class="qlw__bezel-tab"');
    const tabBtn = PAGE_SOURCE.slice(tabStart, PAGE_SOURCE.indexOf("</button>", tabStart));
    expect(tabBtn).not.toContain("onmouseenter");
    expect(tabBtn).not.toContain("onmouseleave");
    expect(tabBtn).toContain("onfocus={() => void bezelPeek()}");
    expect(tabBtn).toContain("onblur={() => void bezelUnpeek()}");
  });

  it("holds the peek through wall-press jitter instead of flickering", () => {
    // Pressing into the screen-edge wall jitters across the tab's hit
    // boundary — the leave only arms a short collapse, and a re-enter before
    // it fires stands it down, so the tab stays expanded (research 0011
    // studies B and D: the wall itself is the target, hide grace catches the
    // flicker).
    const peek = fnBody("bezelPeek");
    expect(peek).toContain("cancelBezelUnpeek()");
    const unpeek = fnBody("bezelUnpeek");
    expect(unpeek).toContain("cancelBezelUnpeek()");
    expect(unpeek).toContain("setTimeout");
    expect(unpeek).toContain("BEZEL_UNPEEK_GRACE_MS");
    // The fired collapse re-guards: an open/toggle/re-enter that landed
    // meanwhile owns the presentation, never the stale timer.
    expect(unpeek).toContain("bezelOpen || bezelPeeking");
    // The toggle takes over hover state — a click never leaves a stale
    // collapse armed behind the panel.
    expect(fnBody("setBezelOpen")).toContain("cancelBezelUnpeek()");
  });

  it("toggles open/closed on tab/peek click through one shared closer", () => {
    expect(PAGE_SOURCE).toContain("onclick={toggleBezelPanel}");
    expect(PAGE_SOURCE).toContain("function toggleBezelPanel()");
    expect(PAGE_SOURCE).toContain("void setBezelOpen(!bezelOpen)");
    expect(PAGE_SOURCE).toContain("function closeBezelPanel()");
    expect(PAGE_SOURCE).toContain("void setBezelOpen(false)");
    const set = fnBody("setBezelOpen");
    expect(set).toContain('placeBezelRect(open ? "open" : "collapsed", false, true)');
    expect(set).toContain("bezelOpen = open");
    // A grip tap without drag toggles like the tab itself.
    expect(PAGE_SOURCE).toContain("else toggleBezelPanel();");
  });

  it("closes on click-outside and Esc, with the dialog owning Esc above the panel", () => {
    expect(PAGE_SOURCE).toContain('"pointerdown", onBezelPointerDown');
    expect(PAGE_SOURCE).toContain("bezelPanelEl?.contains(target)");
    expect(PAGE_SOURCE).toContain("bezelTabEl?.contains(target)");
    expect(PAGE_SOURCE).toContain("bezelGripEl?.contains(target)");
    expect(PAGE_SOURCE).toContain("bind:this={bezelGripEl}");
    expect(PAGE_SOURCE).toContain('"keydown", onBezelKeyDown');
    expect(PAGE_SOURCE).toContain("function onBezelTabKeyDown(");
    // Both Esc paths route to the shared closer; the window-level one yields
    // while the details dialog sits above the panel.
    expect(PAGE_SOURCE).toContain('event.key !== "Escape"');
    expect(PAGE_SOURCE).toContain('event.key === "Escape"');
    expect(PAGE_SOURCE).toContain("if (detailsAction !== null) return;");
  });

  it("never closes on focus loss — no blur/focus wiring reaches the closer", () => {
    for (const line of PAGE_SOURCE.split("\n")) {
      if (/blur|focus/i.test(line)) {
        expect(line, line.trim()).not.toContain("closeBezelPanel");
        expect(line, line.trim()).not.toContain("setBezelOpen(");
      }
    }
    expect(PAGE_SOURCE).not.toContain('addEventListener("blur"');
    expect(PAGE_SOURCE).not.toContain('addEventListener("focusout"');
    expect(PAGE_SOURCE).not.toContain("focusout");
    expect(PAGE_SOURCE).not.toContain("onFocusChanged");
  });

  it("names the tab through the dictionary and pulses once until commitment", () => {
    expect(PAGE_SOURCE).toContain('aria-label={t("dock.bezel.tooltip")}');
    expect(PAGE_SOURCE).toContain('title={t("dock.bezel.tooltip")}');
    expect(PAGE_SOURCE).toContain('{t("dock.bezel.peekHint")}');
    expect(PAGE_SOURCE).toContain('aria-describedby={bezelPulse ? "qlw-bezel-hint" : undefined}');
    expect(PAGE_SOURCE).toContain("qlw__bezel-tab--pulse");
    expect(PAGE_SOURCE).toContain('"sprout.bezel-pulse-seen"');
    expect(PAGE_SOURCE).toContain("markBezelPulseSeen()");
  });

  it("stays keyboard and screen-reader operable", () => {
    // Native button: focusable with free Enter/Space; expanded state announced.
    expect(PAGE_SOURCE).toContain('class="qlw__bezel-tab"');
    expect(PAGE_SOURCE).toContain("aria-expanded={bezelOpen}");
    expect(PAGE_SOURCE).toContain("onkeydown={onBezelTabKeyDown}");
    expect(PAGE_SOURCE).toContain("bind:this={bezelTabEl}");
  });

  it("animates through ADR-0034 tokens and reads geometry from the backend", () => {
    expect(PAGE_SOURCE).toContain("var(--dur-slow)");
    expect(PAGE_SOURCE).toContain("var(--ease-spring)");
    // The no-raw-curve rule governs the stylesheet: motion comes from tokens,
    // never ad-hoc values. (The script's cubic-bezier solver is the exception
    // that proves it — it exists only to ride the --ease-out token value in
    // the JS window tween, with the token numbers as its fallback.)
    const style = PAGE_SOURCE.slice(PAGE_SOURCE.indexOf("<style>"));
    expect(style).not.toContain("280ms");
    expect(style).not.toContain("cubic-bezier");
    expect(style).not.toContain("transition: all");
    expect(PAGE_SOURCE).toContain("getBezelCollapsedWidth");
    expect(PAGE_SOURCE).toContain("getBezelPeekWidth");
    expect(PAGE_SOURCE).toContain("getBezelOpenWidth");
    expect(PAGE_SOURCE).not.toContain("BEZEL_COLLAPSED_WIDTH_PX");
    expect(PAGE_SOURCE).not.toContain("BEZEL_PEEK_WIDTH_PX");
    expect(PAGE_SOURCE).not.toContain("BEZEL_HEIGHT_RATIO");
    expect(PAGE_SOURCE).not.toContain("DOCK_WIDTH_MAX_PCT_BEZEL");
    expect(API_SOURCE).toContain("get_bezel_collapsed_width");
    expect(API_SOURCE).toContain("get_bezel_peek_width");
    expect(API_SOURCE).toContain("get_bezel_open_width");
  });

  it("sequences every bezel window move so a stale placement never wins", () => {
    // One chain owns the window: hover/drag/toggle requests queue in order
    // and only the newest runs — a stale move's late write can never land
    // after the live one (the drag-hold jitter shape).
    expect(PAGE_SOURCE).toContain("function queueBezelMove(");
    expect(PAGE_SOURCE).toContain("++bezelPlaceGen");
    expect(PAGE_SOURCE).toContain("let bezelTail: Promise<void> = Promise.resolve();");
    for (const name of ["placeBezelRect", "placeBezelTab"]) {
      const body = fnBody(name);
      expect(body).toContain("queueBezelMove(");
      expect(body).toContain("gen !== bezelPlaceGen");
    }
    // Size and position issue as one frame unit, and every frame re-checks
    // the generation first — a supersede mid-move stops the next write
    // instead of leaving a half-applied rect behind.
    const tween = fnBody("tweenBezelWindow");
    expect(tween).toContain("win.setSize");
    expect(tween).toContain("win.setPosition");
    expect(tween).toContain("gen !== bezelPlaceGen");
  });

  it("snaps every bezel window move instantly, keeping the token arrival in CSS", () => {    // The window counterpart of the panel's CSS arrival rides --dur-slow
    // with --ease-out read live from the tokens (ADR-0034) — never a
    // duplicated constant — and honors both the app switch and the OS
    // reduced-motion preference like every other surface. Every window move
    // skips the flight: even a 20→60 px per-frame tween stutters on a busy
    // bridge, so the window lands in one move while the CSS tint and panel
    // arrival keep the feedback.
    for (const token of ["--dur-slow", "--ease-out"]) {
      expect(PAGE_SOURCE).toContain(token);
    }
    expect(PAGE_SOURCE).toContain("function tweenBezelWindow(");
    expect(PAGE_SOURCE).toContain("requestAnimationFrame");
    expect(PAGE_SOURCE).toContain("cubic-bezier");
    expect(PAGE_SOURCE).toContain('animation.mode !== "on"');
    expect(PAGE_SOURCE).toContain("prefers-reduced-motion");
    // Drag tracking stays direct (1:1, no tween lag) — every placement
    // snaps via the instant flag and the tab placer never touches the tween.
    expect(fnBody("placeBezelTab")).not.toContain("tweenBezelWindow");
    expect(fnBody("placeBezelRect")).toContain("tweenBezelWindow");
    // One round trip per frame: size and position issue together, so a slow
    // bridge still yields frames instead of a slideshow.
    const tween = fnBody("tweenBezelWindow");
    expect(tween).toContain("Promise.all([");
    expect(tween).toContain("gen !== bezelPlaceGen");
  });

  it("never wedges the chain on a starved frame", () => {
    // rAF never fires for an occluded page: each frame races a timeout and
    // a starved tween lands its end state at once, so covered windows snap
    // correctly instead of queueing every later move behind a frozen frame
    // and flooding through in one teleport.
    expect(PAGE_SOURCE).toContain("function nextBezelFrame()");
    const tween = fnBody("tweenBezelWindow");
    expect(tween).toContain("await nextBezelFrame()");
    expect(tween).toContain("await applyRect(to);");
  });

  it("flips the toggle first so clicks converge instead of swallowing", () => {
    // No in-flight guard to eat rapid clicks: every press flips and queues,
    // the chain converging on the latest intent; only a refused move rolls
    // back, and only when nothing newer landed meanwhile.
    expect(PAGE_SOURCE).not.toContain("bezelToggling");
    const toggle = fnBody("setBezelOpen");
    expect(toggle).toContain("placeBezelRect(open ? ");
    expect(toggle.indexOf("bezelOpen = open")).toBeLessThan(toggle.indexOf("placeBezelRect"));
    expect(toggle).toContain("if (bezelOpen === open) bezelOpen = !open;");
  });

  it("never translates on hover: peek grows in place around the cursor", () => {
    // A stationary cursor must never be uncovered mid-hover — that is the
    // enter/leave flap loop that reads as hover jitter.
    const rect = fnBody("placeBezelRect");
    expect(rect).toContain("hover = false");
    expect(rect).toContain("hover && from");
    expect(rect).toContain("from.x + from.w - width");
  });

  it("derives the drag X from the edge anchor, never re-read", () => {
    const drag = fnBody("onBezelGripPointerDown");
    expect(drag).toContain("startX");
    expect(drag).toContain("placeBezelTab(ratio, g.startX)");
    // A mid-gesture read can land mid-tween and pin a stale edge for the
    // rest of the hold — the anchor is computed from the display instead.
    expect(drag).toContain("getBezelCollapsedWidth()");
    expect(drag).not.toContain("outerPosition()");
    expect(fnBody("placeBezelTab")).toContain("startX ??");
  });

  it("holds the drag hermetic: hover stands down, tremor never floods, release settles", () => {
    // A lagging window slides under the cursor mid-gesture — each boundary
    // cross would otherwise queue a peek/unpeek against the drag's, and
    // alternating writers read as teleporting between the old and new spot
    // (ADR-0019 one writer wins). The held grip owns the window until release.
    expect(fnBody("bezelPeek")).toContain("bezelDragging");
    expect(fnBody("bezelUnpeek")).toContain("bezelDragging");
    const drag = fnBody("onBezelGripPointerDown");
    expect(drag).toContain("cancelBezelUnpeek()");
    // Only the held pointer steers: the moving window synthesizes button-free
    // moves under a stationary cursor that would otherwise drag the tab back
    // toward the old spot on every hold.
    expect(drag).toContain("startPointerId");
    expect(drag).toContain("(ev.buttons & 1) === 0");
    // Tremor below ~1.5 physical px never reaches the bridge; release awaits
    // the trailing placement before persisting, so the stored spot is the
    // shown spot.
    expect(drag).toContain("lastSentRatio");
    expect(drag).toContain("await settle");
    // One rect per placement: size rides along so a drag started peeked can
    // never strand its width on drop.
    const tab = fnBody("placeBezelTab");
    expect(tab).toContain("win.setSize");
    expect(tab).toContain("win.setPosition");
    expect(tab).toContain("getBezelCollapsedWidth");
  });

  it("fails the toggle loudly instead of vanishing the tab", () => {
    // The toggle flips the DOM first — a swallowed placement would strand the
    // open panel in a tab-sized window with no word anywhere. An unresolvable
    // monitor throws so the toggle rolls back and says so.
    expect(fnBody("placeBezelRect")).toContain("bezel monitor is unavailable");
  });

  it("anchors the open panel to its own edge, mirrored per side", () => {
    // Left stays left; right opens leftward from the right edge with the tab
    // on the screen-edge side — never jumping across the screen.
    expect(PAGE_SOURCE).toContain(".qlw--bezel-open.qlw--docked-right");
    expect(PAGE_SOURCE).toContain("flex-direction: row-reverse");
    expect(PAGE_SOURCE).toContain("qlw-bezel-panel-in-right");
    // Window placement anchors to the same edge the tab sits on.
    const rect = fnBody("placeBezelRect");
    expect(rect).toContain('dock.edge === "left" ? info.x : info.x + info.width - width');
  });

  it("dresses the tab in the main-app Button secondary vocabulary", () => {
    const style = PAGE_SOURCE.slice(PAGE_SOURCE.indexOf("<style>"));
    const tab = style.slice(style.indexOf(".qlw__bezel-tab {"), style.indexOf(".qlw__bezel-tab {") + 700);
    expect(tab).toContain("var(--border-strong)");
    expect(tab).toContain("var(--radius)");
    expect(tab).not.toContain("var(--radius-sm)");
    expect(PAGE_SOURCE).toContain("border-color: var(--accent);");
  });

  it("keeps the drag single-flight so a leaked session never doubles", () => {
    // A pointerup that never lands must not let the next press answer moves
    // twice — two sessions parking alternating positions is the hold jitter.
    expect(PAGE_SOURCE).toContain("function endBezelDragListeners()");
    const drag = fnBody("onBezelGripPointerDown");
    expect(drag).toContain("endBezelDragListeners();");
    expect(drag).toContain('"pointercancel", onUp');
    expect(PAGE_SOURCE).toContain('window.removeEventListener("pointercancel", bezelDragUp)');
  });

  it("steers the drag from OS-stable screenY so hold-still never teleports", () => {
    // clientY is viewport-relative: every window move under a stationary OS
    // cursor shifts it (clientY = screenY - windowTop), so the next synthetic
    // held-button move steers back toward the old spot — the hold-still A<->B
    // loop. screenY is OS-stable across window moves, so holding still
    // computes no delta; duplicate-screenY synthetics never reach the bridge.
    const drag = fnBody("onBezelGripPointerDown");
    expect(drag).toContain("startScreenY");
    expect(drag).toContain("lastScreenY");
    expect(drag).toContain("ev.screenY - startScreenY");
    expect(drag).toContain("ev.screenY === lastScreenY");
    expect(drag).not.toContain("ev.clientY - startClientY");
    expect(drag).not.toContain("startClientY");
  });

  it("closes on an outside click that never reaches the webview (ticket 243)", () => {
    // A button-down in another app is invisible to the page, so the native
    // hook reports it; the page then runs the same closer Esc uses.
    expect(PAGE_SOURCE).toContain('listen("bezel-close-requested"');
    expect(PAGE_SOURCE).toContain("closeBezelPanel()).then(");
    // Armed with the open placement, disarmed with every other one — queued
    // with the move so a fast open→close cannot deliver a stale arm.
    const rect = fnBody("placeBezelRect");
    expect(rect).toContain("await syncBezelOutsideClose(kind, gen)");
    const sync = fnBody("syncBezelOutsideClose");
    expect(sync).toContain("gen !== bezelPlaceGen");
    expect(sync).toContain('setBezelOutsideClose(kind === "open")');
    // A route unmount must never leave the hook behind the panel.
    expect(PAGE_SOURCE).toContain("void setBezelOutsideClose(false)");
    expect(API_SOURCE).toContain('invoke<void>("set_bezel_outside_close", { active })');
    // Focus loss stays inert — the hook's own focus path is viewer-only.
    expect(PAGE_SOURCE).not.toContain('addEventListener("blur"');
  });
});

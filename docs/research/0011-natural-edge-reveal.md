# 0011 — Natural edge reveal: seam, pressure and dwell

**Date:** 2026-08-27 · **Replaces:** ad-hoc tuning of reveal delay alone · **Applies to:** Quick Launch dock [Win32 AppBar] on any display count/size/scale

> This note is **code-free and layman-first**. Proper terms are in `[brackets]` only.
> No file names, no constants, no function names — so it stays true if the code moves.
> Tickets `111`/`119` hold the code mapping.

## The problem in one sentence

Deliberate pushes into the docked edge sometimes failed, while accidental brushes along the edge sometimes opened it. The strip also left a thin handle [sliver] on the left edge that looked like an invisible box and could block clicks.

## What people naturally do

- People **slam into screen edges** at speed — edges feel like walls [Fitts's Law — edges are "infinite" targets]. They do not hunt for a tiny handle.
- Dragging **along** an edge [graze] to reach a tab is not trying to open the dock.
- Brushing past an edge and **snapping back** to a close button [×] is not trying to open the dock either — it is aiming at something near the edge and correcting.
- On a setup with two screens, the line where screens touch is **not a wall** — the mouse slips to the other screen. People expect a wall, but there is none.

## Studies

### Study A — The line in the middle is not a wall [monitor seam vs cursor-stop]

- **If you do:** put an auto-hide dock [auto-hide] on the middle line where two screens meet — for example the right edge of the left screen touching the left edge of the right screen, or the bottom of the top screen touching the top of the bottom screen — with more than a tiny corner touching [>1 px overlap].
- **If you do → what happens:** the mouse never stops on that line. It jumps to the neighbour screen in one blink [cursor passes through]. You can make the reveal more sensitive [sensitivity] or faster [dwell] and it still fails — or it flickers open then immediately shuts.
- **This avoids what:** years of trying to fix it by tuning numbers.
- **Study from:** KDE bug that ran 9 years (2015–2024) — comments #1 and #59 say "no amount of sensitivity tuning fixes a shared edge, only moving the dock off that line does"; comment #60 adds the corner case where screens only touch at a single corner [diagonal 1 px touch] — that *is* still a wall. Windows docs say screens must form one touching shape [contiguous region] and the mouse is trapped inside the combined shape [virtual screen].

- **If you do instead:** turn off that middle choice. Let the user pick the outer left or outer right [eligible edge] of that screen. Each screen keeps its own left/right — the top screen's left/right and the bottom screen's left/right are independent [display arrangement]. A small corner touch stays allowed.
- **If you do instead → what happens:** the dock is only offered where the mouse can actually stop. The middle line simply has no dock to trigger.
- **Study from:** same KDE case and Windows virtual-screen docs.

### Study B — People slam into walls [Fitts's Law — infinite edge]

- **If you do:** leave a thin handle [2 px sliver] sticking out while hidden.
- **If you do → what happens:** on the right screen the handle hides in a tiny margin — harmless. On the left screen it sits where your apps start — it can block clicks under it and looks like an invisible box. People also have to hunt for the handle instead of just hitting the wall.
- **This avoids what:** the left-edge invisible box and the hunt.
- **Study from:** NN/g Fitts's Law research — screen edges are "infinite" targets you slam into, not tiny targets you aim at. Windows docs say the app itself must hide [AppBar contract — system never slides it].

- **If you do instead:** when hidden, hide completely with no handle at all [off-screen]. The wall itself is the target — people already know where the wall is.
- **If you do instead → what happens:** no handle to find, no invisible box on any edge. The same feeling on left and right.
- **Study from:** same Fitts's Law + AppBar contract.

### Study C — Sliding along the edge is not intent [direction gating]

- **If you do:** count any movement toward the wall, even if you are mostly moving *along* the wall.
- **If you do → what happens:** sliding up or down along the edge to reach a tab wobbles 1 px toward the wall while you travel 80 px along it — the dock thinks you meant it and pops open.
- **This avoids what:** accidental pops while reaching for something near the edge.
- **Study from:** GNOME's wall [PressureBarrier — `slide > distance → reject`] and dash-to-dock's same rule — when most motion is along the wall, discard that sample.

- **If you do instead:** only count movement that is *more* toward the wall than along it [direction gating], and ignore huge jumps that would cheat [cap per tick].
- **If you do instead → what happens:** slides along the edge add nothing. Only a push *into* the wall counts. No cost when you really do want it.

### Study D — A quick brush and snap-back should not open it [dwell + cancel-if-left]

Three everyday cases:

1. **You move fast then slow down on the wall** — you fling, then ride the wall a moment looking for the dock.
   - **Should open:** yes, after you park on the wall a moment [dwell ~0.2 s], then slide in [slide ~0.18 s].

2. **You slam and hold** — you hit the wall hard and keep pushing.
   - **Should open:** yes, same park time then slide. One slam is enough — you don't need to wiggle.

3. **You slam and yank back to the close button [×] — even a big overshoot and a fast pull-back 80 px in ~30 ms:**
   - **Should never open:** you brushed the wall and are already going back to where you aimed. Opening would cover the button you wanted [edge hijacking].

- **If you do:** open the instant you hit the wall.
- **If you do → what happens:** case 3 covers the × mid-click.
- **Study from:** Windows taskbar's instant `50 ms` open is the ecosystem's example of the defect — Windhawk mod docs it and communities (DisplayFusion, ObjectDock, ButteryTaskbar) complain it hides the button you aimed at. KDE bug 323588 enforces "time to push > time to re-push" [reactivation > activation]. macOS dock waits `~0.2 s` [autohide-delay]. Hover-menu studies (Baymard 300–500 ms, academic `333 ms`) bracket our `~0.2 s` at the low end.

- **If you do instead:** need to *stay* on the wall `~0.2 s` [dwell], and if you leave the wall before that, cancel. Staying inside the now-open strip [strip-contains] keeps it up; a short grace covers reaching for buttons inside. Leaving the strip hides it again.
- **If you do instead → what happens:** case 1 and 2 open after a short hold; case 3 never opens — dwell is aborted on the very next blink. Case 3 with a lot of overshoot is same: the mouse cannot go past the outer wall (it stops at the edge), so the next blink is already away from the wall → cancel.

### Study E — Why we have to check the mouse often [polling]

- **If you do:** wait for the window to tell you the mouse moved [mouse-move event].
- **If you do → what happens:** the web page inside the dock eats the message, so the outer window never hears it — the dock never opens when you are over its content.
- **Study from:** WebView2 feedback threads (#5232, #5250) and Microsoft's note.

- **If you do instead:** ask the system "where is the mouse?" every blink [polling every ~16 ms], the way Explorer's taskbar hook does.
- **If you do instead → what happens:** works whether over the strip or not, on any screen or scale.

## Why the pieces must work together

No single piece separates intent from accident. Each piece catches a different mistake at almost no cost:

- Direction gating catches the *slider along the edge*.
- Dwell catches the *fly-through*.
- Cancel-if-left catches the *slam-and-snap-back* on the next blink.
- Hide grace [hysteresis] catches *flicker* when you overshoot into the now-open strip.

Instant open without a wall — as Windows taskbar does by default — is treated everywhere as a bug, not a feature.

## Sources (primary, not write-ups about them)

- KDE bug 351175 and duplicates — shared-edge comments #1, #59, #60 and 9-year history
- Windows — The virtual screen; About multiple display monitors; Using Application Desktop Toolbars (AppBar contract); ABM_SETAUTOHIDEBAR docs
- GNOME Shell `js/ui/layout.js` PressureBarrier + dash-to-dock `require-pressure`
- KDE KWin screen edges (activation / reactivation delay, 100 px barrier)
- macOS Dock `autohide-delay` defaults
- NN/g Fitts's Law — why edges are infinite
- Baymard hover-intent 300–500 ms
- WebView2Feedback #5232 / discussion #5250
- Windhawk `taskbar-autohide-instant-show` (documents Explorer's 50 ms / 500 ms)

## Mapping to Sprout (only place code is mentioned — stays out of the studies)

- Studies A → ticket `111` (per-screen [per-monitor] left/right offered, middle line disabled with inline reason; top screen left/right independent from bottom screen left/right)
- Studies B–E → ticket `119` (hidden = off-screen, no handle; wall pressure + dwell + direction gating + cancel-if-left; `~16 ms` poll; hide grace)
- Both surfaces (Settings tab and Quick Launch window top bar [edge arrows]) share the same wall rule and same reason string; already-saved middle choice silently moves to the opposite outer edge of that same screen on next dock [auto-migrate, no toast]

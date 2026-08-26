# 0003 — AppBar auto-hide: what the OS contract actually says

**Date:** 2026-08-21 · **Ticket:** 63 (dock auto-hide) · **Question:** who
hides an auto-hide AppBar window — the system or the application? What does
`ABM_SETAUTOHIDEBAR` registration grant, and can message-driven hover
detection work under WebView2?

## Findings

### 1. The application implements hide/reveal itself [official docs]

"Using Application Desktop Toolbars"
([learn.microsoft.com](https://learn.microsoft.com/en-us/windows/win32/shell/application-desktop-toolbars))
defines the behavior entirely in terms of the *appbar's own* actions:

> "An autohide appbar is one that is normally hidden but becomes visible when
> the user moves the mouse cursor to the screen edge with which the appbar is
> associated. **The appbar hides itself again** when the user moves the mouse
> cursor out of the bar's bounding rectangle."

No sentence anywhere assigns motion to the system. The same page's
notification guidance has the app calling `MoveWindow` itself in response to
`ABN_POSCHANGED`. The system never animates or relocates a registered bar.
**This is why Sprout's strip never hid: nothing implemented the motion.**

### 2. What `ABM_SETAUTOHIDEBAR` grants — and what it does not [official docs]

Same page + [`ABM_SETAUTOHIDEBAR`](https://learn.microsoft.com/en-us/windows/win32/shell/abm-setautohidebar):

- Exactly **one autohide appbar per edge**, first-come first-served;
  registering returns `FALSE` when another bar owns the edge.
- "The system automatically maintains the z-order of an autohide appbar
  (within its z-order group only)."
- `ABM_GETAUTOHIDEBAR` exposes the edge owner to other windows.
- An autohide appbar "does not need to register as a normal appbar" via
  `ABM_NEW` at all — registration is about coordination services, not motion.
- Multi-monitor caveat: non-EX messages use "the monitor that contains the
  primary taskbar"; per-monitor registration requires
  `ABM_SETAUTOHIDEBAREX`/`ABM_GETAUTOHIDEBAREX`. Sprout currently sends the
  non-EX forms — correct only where the docked monitor is the primary one.

So registration buys: exclusivity enforcement, z-order help, notifications,
work-area semantics. It does **not** buy hiding. A refusal therefore costs
coordination semantics only — hiding remains fully possible (ticket 63's
"hide anyway" decision is consistent with the documented contract).

### 3. No official mechanism for the visual behavior [official docs]

The docs specify *what* should happen (hidden normally, visible on cursor at
the edge, hide on leaving the bounding rectangle) but provide no API,
sliver-size guidance, or timing. Third-party implementations confirm the
app-driven pattern: WinUI3AppBar implements auto-hide as "`ABM_SETAUTOHIDEBAREX`
+ a DispatcherTimer state machine that slides the bar in/out with ~200 ms
ease-out" ([github.com/tomhoh/WinUI3AppBar](https://github.com/tomhoh/WinUI3AppBar));
the classic VB6 sample polls `GetCursorPos` against a thin-line sliver
([thescarms.com](http://www.thescarms.com/VBasic/appbar.aspx)).

### 4. Raymond Chen / shell internals

Not consulted — question resolved by primary documentation above; no claim
in this note depends on blog-level evidence.

### 5. WebView2 swallows parent mouse input [GitHub issue / official docs]

When the WebView2 controller fills the parent window, "the parent window
never receives the `WM_MOUSEMOVE` message at all", so parent-side
`TrackMouseEvent`/`WM_MOUSELEAVE` cannot fire
([WebView2Feedback #5232](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5232);
see also
[discussion #5250](https://github.com/MicrosoftEdge/WebView2Feedback/discussions/5250)).
The composition-controller docs recommend capture-based tracking for events
that leave the WebView
([CoreWebView2CompositionController](https://docs.dndocs.com/n/Microsoft.Web.WebView2/1.0.2592.51/api/Microsoft.Web.WebView2.Core.CoreWebView2CompositionController.html)).
Consequence for Sprout: hover detection must be cursor-polling
(`GetCursorPos`) from a background loop — the subclassed top-level proc will
never see mouse movement over the strip.

## Implications for ticket 63

1. Custom motion code (slide-to-sliver / edge-touch reveal) is **required** —
   no OS fallback exists on any Windows version covered by these docs.
2. Keep `ABM_SETAUTOHIDEBAR` registration for exclusivity/z-order/work-area
   semantics; treat refusal as informational (banner), never as a blocker
   for hiding.
3. Reveal/hide detection = ~16 ms `GetCursorPos` polling with hysteresis
   (edge-trigger band vs strip-bounds-with-margin), animated by interpolating
   toward the target rect each tick (~180 ms ease-out).
4. Follow-up flag: adopt the `ABM_*EX` variants before claiming multi-monitor
   correctness (Sprout is single-monitor verified today).

## Addendum (2026-08-26) — edge hijacking: accidental reveals & the layered fix

Reported failure: with the dock docked + auto-hide, aiming the cursor at
corner UI near the docked edge (an X button) overshoots into the seam, the
strip slides out over the target — "edge hijacking".

### What Windows itself does [reverse-engineered]

The Windhawk mod *taskbar-autohide-instant-show* hooks Explorer's own reveal
timers and documents the defaults: **unhideDelay 50 ms** (edge-touch → reveal),
**hideDelay 500 ms** (grace before hiding) — instant touch-reveal, no
direction gating (https://raw.githubusercontent.com/ramensoftware/windhawk-mods/main/mods/taskbar-autohide-instant-show.wh.cpp).
Its complaint ecosystem matches ours exactly: a DisplayFusion thread describes
overshooting Chrome's back button at a screen edge, with the dev confirming
no reveal delay exists
(https://www.displayfusion.com/Discussions/View/taskbar-auto-hide-speed/?ID=0197c0a4-6fb7-7027-9555-3fbb44e09551);
ObjectDock users report the dock popping up over intended clicks
(https://forums.stardock.com/446856/hide-show-objectdock-with-keyboard); whole
tools exist only to replace hover-reveal (ButteryTaskbar2 — Win-key/scroll
reveal, https://github.com/LuisThiamNye/ButteryTaskbar2;
auto-hide-taskbar-on-mouse-click — click-at-edge reveal,
https://github.com/Reebey/auto-hide-taskbar-on-mouse-click).

### How mature docks gate the trigger [source-verified]

- **GNOME Shell `PressureBarrier`** (js/ui/layout.js) + dash-to-dock's
  `require-pressure` (threshold default 100): accumulates *toward-edge*
  travel inside a sliding ~1 s window, caps each event (~15 px), and discards
  events whose along-edge component exceeds the perpendicular one
  (`slide > distance` → reject). Grazing along the seam accumulates nothing;
  a deliberate shove crosses the threshold in a few frames
  (https://github.com/GNOME/gnome-shell/blob/main/js/ui/layout.js,
  https://github.com/micheleg/dash-to-dock/blob/master/docking.js).
  dash-to-dock's fallback dwell requires sitting on the edge for `showDelay`,
  cancels if the focused window saw clicks/typing, and re-checks 250 ms after
  triggering.
- **KDE Plasma screen edges**: per-edge enablement, an activation delay
  ("time required for the mouse cursor to be pushed against the edge"), an
  officially enforced reactivation delay greater than it
  (https://bugs.kde.org/show_bug.cgi?id=323588), and an edge barrier that
  physically pushes the cursor back until N px (default 100) of inward travel
  (https://docs.kde.org/stable_kf6/en/kwin/kcontrol/kwinscreenedges/).
- **macOS Dock**: built-in reveal dwell `autohide-delay`, ~0.2 s default
  (https://macos-defaults.com/dock/autohide-delay.html).
- **Hover-intent literature**: Baymard prescribes a 300–500 ms dwell for
  hover-triggered dropdowns (https://baymard.com/blog/dropdown-menu-flickering-issue);
  academic value ≈333 ms. NN/g's Fitts-law piece explains why edges suffer:
  edges are infinite targets users slam into at speed
  (https://www.nngroup.com/articles/fitts-law/).

### Term audit (borrowed vocabulary checked against sources)

- "**Triangle filter**" — an image-resampling kernel (box filter convolved
  with itself; dspguide.com ch. 15). No credible pointer-intent usage found.
  Dropped.
- "**Directional velocity tracking**" — not an established UI term; the
  concept exists concretely as PressureBarrier above.
- "**Push past seam threshold**" — KDE's edge barrier / GNOME's pressure
  accumulation.
- Hysteresis — established; Sprout already applies it on the hide side.

### Layman's guide: why the layers combine

No single trick separates "deliberate push into the edge" from accidental
contact. Each layer below kills a different false-positive class at almost no
cost to genuine use:

| Layer | Catches | Genuine-user cost |
|---|---|---|
| Direction gating (accumulate toward-edge travel; discard along-edge-dominant motion) | the grazer sliding along the seam | none |
| Reveal dwell (~200 ms sustained in the trigger band) | the fly-through overshooting and rebounding | ~200 ms once |
| Cancel-if-left (dwell aborts when the cursor exits the band) | same class, second net | none |
| Hide hysteresis/grace (already present) | flicker; protects a user who overshot INTO the revealed strip | none |

Instant touch survives only in Windows' taskbar — whose surrounding
ecosystem treats it as a defect.

### Decisions for Sprout (constants stay single-sourced in constants/window.rs)

**Scope rule: the mechanism is topology-independent** — any Windows device,
any monitor count, size, scale factor, dock edge. All reveal/hide geometry is
derived at runtime from the docked monitor's own rect (the same rect the
AppBar registration maintains); no constant encodes a resolution, DPI, or
monitor count, and every poll sample compares cursor coordinates against the
monitor the dock currently occupies. The dev-machine diagnostics (one
2560×1920 panel @150%, dock registered right + auto-hide, work area pulled to
x=2558 by Sprout's own registration) merely explain the *reported* left/right
asymmetry — on the docked edge the cursor can enter the reserved sliver where
nothing renders, on a bare edge it clamps flush; AppBar reservation +
pointer-hotspot geometry, not a code defect. Diagnostics are evidence for the
symptom, never inputs to the design.

1. Trigger band shrinks 8 px → the sliver width itself (derived from
   `AUTOHIDE_SLIVER_PX`; the invisible reserved zone *is* the target).
2. Reveal = direction-gated accumulation **and** ~200 ms dwell; the dwell
   cancels when the cursor leaves the band. Hide hysteresis unchanged.
3. No multi-monitor special case is needed: crossing into (or across) the
   band from a neighboring monitor is toward-edge travel, but the cursor
   exits the 2 px band within a frame or two, so cancel-if-left aborts the
   dwell before it completes — routine cross-monitor transit never reveals
   the dock. This is exactly the failure mode behind dash-to-dock's
   "sticky edge" reports between monitors
   (https://github.com/micheleg/dash-to-dock/issues/1983); per-monitor
   geometry plus the layered gates answer it.
4. Two knobs under Settings → Advanced (Disclosure section): reveal delay
   (default 200 ms) and sensitivity/threshold (default GNOME-equivalent).

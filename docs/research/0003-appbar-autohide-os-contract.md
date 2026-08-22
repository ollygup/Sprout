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

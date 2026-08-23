# 68 — Frontend shared-module extraction

**What to build:** Three pieces of logic that today exist as byte-identical
copies across routes and dialogs each get one home under `$lib`, and every
call site consumes it:

1. **Slug derivation** — three identical copies (presets page, product form
   dialog, preset form dialog) collapse into one helper inside the existing
   display-helpers module.
2. **Flash notices** — five route-local "show a transient notice, auto-clear
   on timeout" copies become one tiny store module. The plan page's notice
   intentionally lives longer than the others; preserve each page's exact
   current timeout via a parameter — unifying durations is a behavior change
   and is out of scope.
3. **Run-status vocabulary** — status ordering, badge-tone mapping, result
   grouping, and the mismatch-note check exist twice (plan + history pages)
   with one drift point already visible: history checks a magic string inline
   where plan uses a named predicate. One pure module beside the shared type
   definitions owns all four.

All bodies move verbatim — copy-out-and-import, no redesign.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] Exactly one slugify / flash store / run-status vocabulary exists; zero
      copies remain in routes or components
- [ ] Per-page notice timeouts preserved exactly (incl. plan's longer duration)
- [ ] History's inline mismatch-note literal replaced by the shared named
      predicate (same string, same semantics)
- [ ] `vitest run` green, `npm.cmd run check` 0 errors

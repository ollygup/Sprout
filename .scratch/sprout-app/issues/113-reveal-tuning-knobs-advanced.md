# 113 — Reveal tuning knobs under Settings → Advanced

**What to build:** Two quiet knobs — reveal delay (ms) and reveal sensitivity threshold — become validated settings surfaced under an Advanced disclosure inside the Dock section, defaulting to the shipped values from the reveal gate. The driver honors them on subsequent evaluations, so users can trade snappiness for false-positive immunity without touching code.

**Blocked by:** 112 — layered autohide reveal gate.

**Status:** done

- [x] Both knobs validate and clamp to sane ranges following the established settings pattern; broken stored values fall back to defaults
- [x] Defaults equal the shipped gate constants; changing a knob changes driver behavior without restart
- [x] Knobs live under a collapsed-by-default Advanced disclosure using the shared Disclosure primitive; hidden entirely when the dock feature surface doesn't apply
- [x] Labels describe both states plainly (information scent); tokens/components reused only
- [x] Validator/clamp tests added; `svelte-check` clean

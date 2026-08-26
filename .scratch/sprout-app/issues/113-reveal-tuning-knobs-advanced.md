# 113 — Reveal tuning knobs under Settings → Advanced

**What to build:** Two quiet knobs — reveal delay (ms) and reveal sensitivity threshold — become validated settings surfaced under an Advanced disclosure inside the Dock section, defaulting to the shipped values from the reveal gate. The driver honors them on subsequent evaluations, so users can trade snappiness for false-positive immunity without touching code.

**Blocked by:** 112 — layered autohide reveal gate.

**Status:** ready-for-agent

- [ ] Both knobs validate and clamp to sane ranges following the established settings pattern; broken stored values fall back to defaults
- [ ] Defaults equal the shipped gate constants; changing a knob changes driver behavior without restart
- [ ] Knobs live under a collapsed-by-default Advanced disclosure using the shared Disclosure primitive; hidden entirely when the dock feature surface doesn't apply
- [ ] Labels describe both states plainly (information scent); tokens/components reused only
- [ ] Validator/clamp tests added; `svelte-check` clean

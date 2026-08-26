# 111 — Per-monitor dock preferences in Settings

**What to build:** With two or more displays connected, Settings' Dock section gains a "Per-monitor" area listing every connected display (friendly label + resolution) with its own Edge and Mode selects, writing through the existing validators into the per-display dock memory. Single-display machines see nothing new — the area exists only when it has content. Global default controls remain above it as the fallback path.

**Blocked by:** 110 — stable monitor identity (EDID).

**Status:** ready-for-agent

- [ ] A command enumerates connected displays with label, resolution, and identity
- [ ] Get/set commands read and write per-display Edge and Mode through the existing validators and persistence helpers
- [ ] The Per-monitor area appears only when more than one display is connected; a single display shows today's unchanged Dock section
- [ ] Saved per-display choices take effect on the next dock/restore for that display
- [ ] Rows are keyboard-operable and labeled; styling uses existing tokens/components only
- [ ] `svelte-check` clean; backend round-trip tests cover set → load per identity

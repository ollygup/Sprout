# 17 — Quick install (backend)

**What to build:** The backend capability for installing a single Product without composing a Preset. A new command synthesizes one default Requirement from a Product (latest version policy, its winget step, its default env wiring) and starts a run directly; History labels such runs "Quick install — {product}". No preset is created or required. The frontend entry point and rendering land in ticket 21.

**Blocked by:** 13 — Product authoring with winget search; 15 — Live-linked requirements; 16 — Honest run outcomes

**Status:** done

- [x] Backend command takes a Product id, synthesizes the default Requirement (latest policy, winget step, default env wiring) and starts the standard run flow
- [x] Run records label quick installs "Quick install — {product}" and render through the same outcome tiers as preset runs
- [x] Backend tests cover: synthesis from product defaults, run start, outcome persistence, and a product without a usable step producing a clear error (never silent success)
- [x] `cargo test` green, `cargo check` clean
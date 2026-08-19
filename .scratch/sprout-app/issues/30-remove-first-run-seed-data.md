# 30 — Remove first-run seed data

**What to build:** Fresh installs open to an empty Library. The 14 seeded catalog entries, the seeding guard, and the seeding tests are gone — Products exist only after the user adds them from the live winget registry search. Existing development data on this machine is wiped (pre-release). A record (ADR) explains why nothing is seeded.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] A fresh database initializes with zero Products and no seeding mechanism (no `seeded` guard remains)
- [x] All backend tests pass without seed fixtures; tests that previously relied on seed Products create their own
- [x] Docs no longer reference the 14-entry seed (AGENTS.md verification helper, specs, parity record)
- [x] Dev machine database wiped; the app launches to an empty Library with the standard empty state
- [x] ADR-0008 written (no first-run seed; Products come only from live winget search)
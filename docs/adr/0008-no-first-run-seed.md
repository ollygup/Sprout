# No first-run seed — the Library starts empty

Fresh installs open to an empty Library: the 14 starter catalog entries, the `seeded` meta guard, and the seeding mechanism are gone. Products exist only after the user adds them from the live winget registry search (`search_winget`) or by hand (`create_product`).

## Why

The first-run seed was a carry-over from the legacy catalog: it gave a blank-but-warm canvas so a new user could compose a preset without typing winget IDs from memory. In practice it shipped the wrong product set to everyone — a stale, opinionated snapshot that users had to curate before their library was theirs. The live winget registry search (ticket 13) now fills that gap better: adding a product is a search-pick, so nobody types winget IDs from memory, and every entry in the Library is deliberate, current, and user-chosen. A seeded catalog also implied catalog maintenance in the app, which Sprout does not want to be.

## Decisions

- **Nothing is seeded**: `init_at` creates the schema and nothing else; the `seed` module is deleted and `seed_if_needed` (and its `seeded` meta flag) no longer exists. A fresh database has zero Products.
- **Products come from the user**: the Add-product dialog's live winget registry search is the primary path; manual entry remains for software outside the registry (ADR: none — spec decision 13).
- **No cleanup migration**: pre-release only. Existing development databases are wiped; this ADR is the record of why.
- **`meta` table stays**: it still holds the settings keys (default timeout, log retention) — only the seed guard is removed.

## Consequences

- Fresh installs open to the standard empty state ("Nothing planted yet"), which is honest about a library that is actually empty.
- Tests that previously leaned on seed fixtures now create their own Products (db.rs `make_product`), so nothing in the suite depends on starter rows.
- Docs and verification helpers no longer reference a 14-entry seed; the parity record (ticket 10) is untouched because it compared legacy vs Sprout *behavior*, not seed contents.
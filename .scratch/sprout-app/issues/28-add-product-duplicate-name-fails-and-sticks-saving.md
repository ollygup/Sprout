# 28 — Add product: duplicate name fails with a generic error and the dialog sticks on "Saving…"

**What to build:** Two defects in the add/edit product flow (user repro: searched "dbeaver", picked the match, clicked Save → "Couldn't add that product. Please try again." while the Save button stayed "Saving…" forever and Cancel was greyed out):

1. **Duplicate check is missing.** The new product's id is `slugify(name)`; the seed catalog already owns id `dbeaver`, so adding "DBeaver" violates the `products.id` PRIMARY KEY and `create_product` errors. `validate_product` (`src-tauri/src/db.rs`) checks only blank fields, never name/id uniqueness, and `+page.svelte:save()` swallows the backend message into the generic "Couldn't add that product. Please try again." The check must happen before the insert, with a friendly message (mirroring `create_preset`'s ConstraintViolation handling).
2. **The dialog's saving state never resets on failure.** `ProductFormDialog.submit()` sets `saving = true` and calls `onsave(...)` without awaiting; on failure the parent only sets `error`, so `saving` stays `true` — the Save button spins forever and Cancel is disabled. `submit()` must be async, `await` the save, and reset `saving` in a `finally`.

**Blocked by:** — (bug found in product-authoring follow-up; frontend + db.rs)

**Status:** done — 163 backend tests green (incl. two new duplicate/rename tests), svelte-check 0 errors; DBeaver repro covered by test

- [x] `db::create_product` rejects an existing product id **or** a case-insensitive matching name with a friendly error ("A product named '…' already exists." style) before the insert
- [x] `db::update_product` applies the same name check excluding the product itself (renaming to another product's name must fail; keeping one's own name must pass)
- [x] `+page.svelte:save()` surfaces the backend error string in the dialog instead of discarding it (generic wording kept only as a fallback)
- [x] `ProductFormDialog` resets `saving = false` after the save settles — success unmounts the dialog, failure leaves it open with Cancel enabled and the error shown
- [x] Regression tests in `db.rs`: duplicate id → friendly Err; duplicate name (different case) → friendly Err; rename onto another product's name → Err; rename keeping own name → Ok
- [x] `cargo test` green in `src-tauri\` (163 passed, incl. the two new tests), `npm run check` 0 errors; the DBeaver repro is covered by `duplicate_product_id_or_name_rejected_with_friendly_error` (same id + same name against the seed, friendly message asserted); working copy synced to the share (add/update robocopy, `/L` shows Copied: 0)

## Diagnosis record (ticket 28, 2026-08-16)

### Symptom (user)

After searching for dbeaver and selecting it, clicking Save shows "Couldn't add that product. Please try again."; the Save button stays loading forever and Cancel is greyed out.

### Root cause

- `ProductFormDialog.svelte:220` sets `saving = true` then fires `onsave(...)` (a promise) without awaiting. `+page.svelte:save()` catches the rejection and sets only `error`; `formOpen` stays true and `saving` is never reset (it resets only in the dialog's `open` effect) → permanent "Saving…" + disabled Cancel.
- The rejection itself is the `products.id` PRIMARY KEY violation: the seed already contains `dbeaver` (verified via the seed catalog), and the dialog's new-product id is `slugify(name)` → "DBeaver" → `dbeaver`. `validate_product` has no uniqueness rule, and the generic message hides the real cause from the user.

### Fix direction (agreed at diagnosis)

Friendly duplicate pre-check in `db::create_product` / `update_product` (id or case-insensitive name), surface the backend message in the dialog, and reset `saving` via an awaited `onsave` in a `finally`.
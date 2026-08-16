---
id: TASK-48
title: Include OpenFoodFacts in free-text food search
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-16 00:28'
updated_date: '2026-08-16 01:26'
labels: []
dependencies: []
priority: medium
type: feature
ordinal: 53000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
search_food currently only queries OpenFoodFacts for barcode inputs; free-text queries search custom foods + USDA FDC only (documented in the operation description but a real coverage gap — OFF's catalog of branded/packaged products is much richer than USDA's for everyday food names).

Fix: add OffClient::search_products(query) hitting the same v2 product API already used by lookup_barcode (/api/v2/product?search_terms=...&page_size=N&fields=...), returning the existing Product shape. Merge its results into SearchFood::search_free_text as a third source alongside USDA, using the established pattern: fetch a small buffer (~10), upsert each into the local catalog via upsert_catalog_food("OpenFoodFacts", ...), then merge_candidates dedupes and caps at 5. An OFF failure must log a warning and NOT fail the search (same fault tolerance as USDA). Update the operation description to reflect that free-text queries now also search OpenFoodFacts.

User confirmed 2026-08-16: implement now.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 OffClient::search_products() queries the OFF v2 product endpoint with search_terms and returns parsed Products using the existing fields list (barcode, name, serving size, per-100g macros)
- [x] #2 Free-text search_food merges OFF results with custom foods and USDA FDC results; merged output still dedupes and caps at 5 candidates
- [x] #3 An OpenFoodFacts request failure logs a warning and does not fail the search (USDA/custom results still returned); same when OFF returns zero products
- [x] #4 Operation description updated so it no longer says free-text queries skip OpenFoodFacts
- [x] #5 Tests cover: OFF hits appear in merged results with source OpenFoodFacts, OFF failure path falls through to other sources, empty OFF response adds nothing
- [x] #6 CI green: cargo fmt --check, clippy -D warnings, nextest --all-features --workspace, cargo test --doc, rustdoc build
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation plan (recorded 2026-08-16)

1. **`nom-core/src/client/off.rs`** — new `OffClient::search_products(query: &str, limit: usize) -> Result<Vec<Product>, OffError>`:
   - GET `{base}/api/v2/product?search_terms={query}&page_size={limit}` with the SAME `fields` list as `lookup_barcode` (reuse it via a shared const/vec helper so the two endpoints can't drift).
   - Parse the paginated response shape (`{ status, count, products: [...] }`) into `Vec<Product>`; `status != 1` or empty → `Ok(vec![])`.
   - Skip products with no barcode (`code`) — upsert needs an external_id.
   - Unit tests against a mock server (existing test pattern in off.rs): happy path returns N products, zero-products page returns empty vec.
2. **`nom-core/src/food/mod.rs` SearchFood::search_free_text** — add OFF as a third source after USDA:
   - `self.off_client.search_products(query, 10).await`, on `Err(e)` `tracing::warn!` + continue (mirrors the USDA warn-and-continue blocks); on `Ok(products)` map each through `extract_off_macros` + `upsert_catalog_food(conn, "OpenFoodFacts", &product.code, ...)` and push candidates with `source: "OpenFoodFacts"`.
   - Existing `merge_candidates(all_candidates, 5)` handles dedupe/cap unchanged.
3. **Operation description** — update to: free-text queries search custom foods, OpenFoodFacts, and USDA FDC.
4. **Tests (food/mod.rs)** — extend the existing mock-based search tests:
   - OFF hit appears in merged results with `source == "OpenFoodFacts"` (mock both OFF and USDA endpoints, assert both sources present under cap).
   - OFF endpoint returns 500 → search still succeeds with USDA/custom results.
   - OFF returns empty products → no OFF candidates added.
5. CI suite green; commit with Task-Id trailer.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Endpoint correction (found during live-API verification): /api/v2/product?search_terms=... REJECTS pure text queries (returns status 0, "no code or invalid code"). Text search lives at the legacy /cgi/search.pl?search_terms=...&search_simple=1&action=process&json=1. OffSearchResponse.status is Option<u8> because that endpoint omits status on success.

The legacy endpoint mishandles `fields` with nutriments:* sub-fields (replaces the real nutriments object with an empty nutriments_estimated), so search_products sends NO fields parameter. lookup_barcode keeps field scoping (works correctly on the v2 endpoint).

OFF's edge intermittently returns an HTML 'Page temporarily unavailable' page under load; the warn-and-continue path absorbed this during live smoke testing (search still succeeded via other sources).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
OpenFoodFacts is now a third source for free-text `search_food`, alongside custom foods and USDA FDC.

## Changes
- **`nom-core/src/client/off.rs`**
  - New `OffClient::search_products(query, limit)` — text search against OpenFoodFacts, returning parsed `Product`s in OFF relevance order; products without a barcode are dropped (the catalog upsert needs one as external id); empty pages are `Ok(vec![])`.
  - Shared `PRODUCT_FIELDS` const now backs both the barcode lookup and (where honored) search.
  - `Product` gained an optional `code` field (per-item barcode).
  - New `OffSearchResponse` for the paginated shape; `status: Option<u8>` because the search endpoint omits it on success.
  - 4 new client tests (success + code-less filtering, empty page, explicit status 0, unexpected status).
- **`nom-core/src/food/mod.rs`**
  - `search_free_text` now also queries OFF (buffer of 10), upserts hits into the local catalog via `upsert_catalog_food("OpenFoodFacts", ...)`, and merges them AFTER the USDA results — strictly additive ordering (Custom > USDA > OFF), so existing users see identical output when ≥5 candidates already exist.
  - OFF failures log `tracing::warn!` and never fail the search (same fault tolerance as USDA).
  - Operation description updated: free-text queries search custom foods, OpenFoodFacts, and USDA FDC.
  - 3 new operation tests (merged ordering with macros/serving size, 500 fall-through to other sources, empty OFF adds nothing).

## Endpoint findings (live-API research)
- `/api/v2/product?search_terms=...` rejects pure text queries (`status: 0`, "no code or invalid code"); text search lives at the legacy `/cgi/search.pl` endpoint.
- That endpoint omits `status` on successful responses.
- Sending `fields` with `nutriments:*` sub-fields makes it replace the real `nutriments` object with an empty `nutriments_estimated` — so search requests omit `fields` entirely.
- OFF's edge intermittently serves an HTML "Page temporarily unavailable" page; warn-and-continue absorbs it (observed live).

## Verification
- Full CI green: fmt --check, clippy -D warnings, nextest 288/288 (--all-features --workspace), doctests, rustdoc build.
- Live end-to-end: `nom-mcp search_food --query "chocolate protein bar"` returned 5 OpenFoodFacts candidates with correct per-100g macros (e.g. "Protein Bars Cocoa Hazelnut" 424.4 kcal/100g, P=16.1, C=32.6, F=21.7 — matching raw API data).
<!-- SECTION:FINAL_SUMMARY:END -->

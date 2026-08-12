---
id: TASK-2.13
title: 'Food operations and resolution workflow (search_food, create_custom_food)'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:24'
updated_date: '2026-08-12 05:18'
labels:
  - planned
dependencies:
  - TASK-2.5
  - TASK-2.7
  - TASK-2.8
  - TASK-2.9
  - TASK-7
  - TASK-8
type: feature
ordinal: 32000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
search_food(query): barcode-shaped (all-digit) queries route to OpenFoodFacts only; everything else routes to local Custom Foods (case-insensitive substring match, searched first) + USDA FDC, merged into one list capped at 5 combined candidates. Every candidate is upserted into the local foods cache as part of the call (full nutrient snapshot, no auto-refresh) — searching IS resolving, food_id is immediately usable. Each candidate carries food_id, name, source, and its full cached nutrient snapshot (no separate get_food_details tool).

create_custom_food(name, serving_size:{quantity,unit}, nutrients): nutrients given per one serving. No server-side dedup — reuse relies entirely on search_food's substring match; tool descriptions must instruct 'search before creating'.

Fallback workflow for tool descriptions (LLM-orchestrated, not server logic): barcode miss falls through to a free-text search_food retry; free-text/dish miss triggers per-ingredient decomposition with LLM-judgment collapse to a whole-dish Custom Food when mostly uncatalogued; barcode/label photos are transcribed/extracted by the LLM itself, not a tool.

Edit/delete: no delete_food operation exists in v1 — Foods are never hard-deleted.

See doc-5 §5, §6, §13.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 search_food routes barcode-shaped queries to OFF only and free-text queries to Custom+USDA merged/capped at 5
- [x] #2 every returned candidate is upserted into the foods table as part of the search_food call
- [x] #3 create_custom_food stores nutrients on a per-serving basis
- [x] #4 no delete_food Operation exists
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

### File structure
New module: `nom-core/src/food/mod.rs` — contains both operations plus shared helpers. Exported from `lib.rs`.

### Step 1: Create food/mod.rs with shared types

**FoodCandidate struct** — the unified response item returned by search_food:
- Fields: `food_id: i64`, `name: String`, `source: String`, `calories_per_100g: f64`, `protein_g_per_100g: f64`, `carbs_g_per_100g: f64`, `fat_g_per_100g: f64`, `fiber_g_per_100g: f64`, `serving_size_g: Option<f64>`
- Derive `serde::Serialize` for JSON output

**Nutrient conversion helper** — per-serving-to-per-100g:
- Formula: `(nutrient_at_serving * 100.0) / serving_size_g`
- If serving_size_g == 0.0, return nutrient as-is (store raw value with warning logged)

**Barcode detection** — no regex dependency needed:
- `query.chars().all(|c| c.is_ascii_digit()) && !query.is_empty()`

**DB upsert helper**:
- For catalog sources (OpenFoodFacts, USDA_FDC): `INSERT INTO foods (...) VALUES (...) ON CONFLICT(source, external_id) DO UPDATE SET name=?, calories_per_100g=?, ... RETURNING id`
- For Custom: plain `INSERT INTO foods (source, name, ...) VALUES ('Custom', ?, ...) RETURNING id` (no conflict possible since external_id is NULL)

### Step 2: SearchFood Operation

**SearchFoodRequest** — derives JsonSchema + Deserialize:
- Field: `query: String`
- `.from_json(args)` method

**execute_json logic**:
1. Parse request from args
2. Open DB connection via `Connection::open()`
3. If barcode-shaped query: call OffClient lookup only
   - On success: extract per-100g macros from Product (prefer _100g fields), upsert into foods table, wrap in FoodCandidate
   - On not-found: return empty list
4. Else (free-text): parallel fan-out of two searches
   - Custom Foods: SQL substring match `WHERE source='Custom' AND LOWER(name) LIKE '%' || LOWER(?) || '%' LIMIT 5`
   - USDA FDC: search_foods(query, 1) then batch-fetch details via get_foods_batch(), extract macros, upsert each
5. Merge results: Custom-first ordering, deduplicate by name (case-insensitive), cap total at 5
6. Return JSON array of FoodCandidate objects

### Step 3: CreateCustomFood Operation

**CreateCustomFoodRequest** — derives JsonSchema + Deserialize:
- Fields: name, serving_size {quantity, unit}, nutrients {calories, protein_g, carbs_g, fat_g, fiber_g}
- Validate: serving_size.quantity > 0, reject if zero or negative

**execute_json logic**:
1. Parse request
2. Convert all nutrients from per-serving to per-100g using helper
3. Insert into foods table (source='Custom', external_id=NULL)
4. Return FoodCandidate with the new food_id and full snapshot

### Step 4: Register operations

- Add `pub mod food;` to lib.rs
- In server startup code (where OperationRegistry is built), register both operations with their clients injected

### Step 5: Tests

**Unit tests** (pure logic):
- Barcode detection: digit-only strings true, mixed false, empty false
- Per-serving conversion: basic math verification, zero-serving edge case
- OFF product macro extraction: prefer _100g fields when available

**Integration tests** (temp DB + wiremock):
- search_food barcode path: wiremock OFF stub, verify upsert + candidate shape
- search_food free-text path: wiremock USDA stub + pre-seeded Custom food, verify merge, cap at 5
- create_custom_food: verify insert, per-serving to per-100g conversion, returns food_id
- Upsert idempotency: calling search_food twice with same query should not duplicate rows

### Key design decisions
- No delete_food operation (doc-5 section 13)
- Searching IS resolving — candidates carry food_id immediately usable by log_meal
- Custom-first ordering in merged results preserves user-defined foods
- All nutrient storage normalized to per-100g invariant
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented food/mod.rs with SearchFood (barcode routes to OFF, free-text routes to Custom+USDA merged/capped at 5) and CreateCustomFood operations. All candidates upserted into foods table during search. Nutrients stored per-100g with conversion from per-serving input. No delete_food operation exists (per doc-5 §13). Tests include unit tests for barcode detection, nutrient conversion, OFF macro extraction, merge/dedup logic, plus integration tests with wiremock + temp DB for barcode path, free-text path, custom-only results, USDA merge, upsert idempotency, create_custom_food per-100g conversion, non-gram units, and validation rejections.
<!-- SECTION:NOTES:END -->

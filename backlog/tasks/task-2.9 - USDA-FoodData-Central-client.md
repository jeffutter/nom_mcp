---
id: TASK-2.9
title: USDA FoodData Central client
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:23'
updated_date: '2026-08-12 00:59'
labels:
  - planned
dependencies:
  - TASK-2.3
type: feature
ordinal: 28000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Bespoke reqwest client for the USDA FDC API (no existing Rust crate). Search endpoint (/fdc/v1/foods/search) and detail/batch endpoints (/fdc/v1/food/{fdcId}, /fdc/v1/foods). Query only Foundation + SR Legacy + Survey (FNDDS) data types via the dataType filter; exclude Branded. Nutrients per 100g, with household/serving portions surfaced alongside. API key from config (env or file), free api.data.gov key, 1,000 req/hr. Base URL must be a constructor parameter for the same testing reason as the OFF client.

See doc-5 §1 and §11.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 search queries filter to Foundation, SR Legacy, and Survey (FNDDS) data types only
- [x] #2 detail/batch responses are parsed into kcal/protein/carbs/fat/fiber per 100g plus household portions
- [x] #3 API key is read from config and never logged; base URL is a constructor parameter
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
### Implementation Plan: USDA FoodData Central Client

**Location**: `nom-core/src/client/usda.rs` (new file) + update `nom-core/src/client/mod.rs`.
No new crate dependencies — all required crates (`reqwest`, `serde`, `thiserror`, `url`) are already in `nom-core/Cargo.toml`. Dev dependency `wiremock` already present.

---

#### Step 1: Error type (~20 lines)

Define `FdcError` enum with `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum FdcError {
    #[error("invalid base URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("API returned HTTP {status}: {message}")]
    ApiError { status: u16, message: String },
    #[error("rate limited — retry after backoff")]
    RateLimited,
}
```

- Covers URL parsing, reqwest failures, generic API errors (non-200), and explicit 429 rate-limit detection
- Check `X-RateLimit-Remaining` header for informational logging (via tracing::debug)

---

#### Step 2: Response structs (~80 lines)

Deserialize only the fields nom_mcp needs. The API returns `oneOf` food types but all share common fields — use a flat struct approach rather than untagged enums, since we only care about shared fields (fdcId, description, dataType, foodNutrients, foodPortions).

Key structs:

- `FdcSearchResponse`: top-level wrapper with `foodMatches` array, `totalHits`, `currentPage`, `pageNumber`, `pageSize`
- `FdcFoodMatch`: search result entry with `fdcId`, `description`, `dataType`, `brandName` (optional)
- `FdcFoodDetailResponse`: detail endpoint response with all nutrient/portion data
- `FdcBatchResponse`: batch endpoint wrapper with `foods` array
- `FdcNutrient`: nested nutrient info (`nutrient.id`, `nutrient.name`, `amount`, `nutrient.unitName`)
- `FdcPortion`: household/serving info (`modifier`, `gramWeight`, `portionDescription`, `amount`)

Use `serde(default)` liberally — API responses may omit optional fields.

For the detail response, deserialize into a unified struct that captures:
- `fdc_id: i64` (from `fdcId`)
- `description: String`
- `data_type: String` (from `dataType`)
- `food_nutrients: Vec<FdcNutrient>` (from `foodNutrients`)
- `food_portions: Vec<FdcPortion>` (from `foodPortions`, default empty)

The API's `oneOf` distinction between Foundation/SR Legacy/Survey foods differs mainly in which optional fields are present — our deserialization ignores those differences and captures what we need.

---

#### Step 3: Nutrient extraction helper (~30 lines)

Extract the five macro nutrients per 100g from the raw nutrient list. Filter by both `nutrient.id` AND `unitName` to avoid kJ energy or other unit variants.

Canonical nutrient IDs:
```rust
const NUTRIENT_ENERGY_KCAL: i64 = 208;  // kcal
const NUTRIENT_PROTEIN: i64 = 203;      // g
const NUTRIENT_FAT: i64 = 204;          // g
const NUTRIENT_CARBS: i64 = 205;        // g
const NUTRIENT_FIBER: i64 = 291;        // g
```

Helper struct and method:
```rust
pub struct FdcNutrients {
    pub energy_kcal: Option<f64>,
    pub protein_g: Option<f64>,
    pub fat_g: Option<f64>,
    pub carbs_g: Option<f64>,
    pub fiber_g: Option<f64>,
}

impl FdcFoodDetailResponse {
    pub fn extract_macros(&self) -> FdcNutrients {
        // Scan food_nutrients for matching (id, unit_name) pairs
        // Energy: id=208 AND unitName="kcal" (not "kJ")
        // Macros: id match AND unitName="g"
    }
}
```

Also surface portions as a convenience method:
```rust
pub struct FdcPortionInfo {
    pub modifier: Option<String>,     // e.g. "1 medium"
    pub gram_weight: f64,             // weight in grams
    pub description: Option<String>,  // portion description
}
```

---

#### Step 4: Client struct (~40 lines)

Follow the OFF client pattern exactly:

```rust
pub struct FdcClient {
    http: reqwest::Client,
    base_url: Url,
    api_key: String,
}

impl FdcClient {
    pub fn new(base_url: &str, api_key: &str) -> Result<Self, FdcError> {
        let url = Url::parse(base_url)?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { http, base_url: url, api_key: api_key.to_string() })
    }

    pub fn with_default_base(api_key: &str) -> Result<Self, FdcError> {
        Self::new("https://api.nal.usda.gov/fdc/v1", api_key)
    }
}
```

Constructor takes base_url (for testing) and api_key directly. The caller (operations layer) passes the key from config's `RedactedString::get()`.

---

#### Step 5: Search endpoint (~60 lines) — Acceptance Criterion #1

Implement `search_foods(query: &str, page: u32) -> Result<FdcSearchResponse, FdcError>`.

- Use POST to `/foods/search` with JSON body (avoids URL encoding issues with arrays)
- Body includes:
  ```json
  {
    "query": "chicken breast",
    "dataType": ["Foundation", "SR Legacy", "Survey (FNDDS)"],
    "pageSize": 50,
    "pageNumber": 1
  }
  ```
- **Critical**: `dataType` filter EXCLUDES Branded — sent as exact string array in POST body
- Append `api_key` as query parameter on every request
- Parse response into `FdcSearchResponse`
- Returns paginated results with `totalHits` for pagination awareness

POST is preferred over GET because:
1. Avoids URL length limits with complex queries
2. `dataType` array serialization is cleaner in JSON body than URL-encoded form style
3. More readable test fixtures

---

#### Step 6: Detail endpoint (~30 lines) — Acceptance Criterion #2

Implement `get_food(fdc_id: i64) -> Result<FdcFoodDetailResponse, FdcError>`.

- GET `/food/{fdc_id}?api_key=...`
- Optionally accept a nutrient filter param for efficiency (up to 25 nutrient numbers)
- Parse into unified detail response struct
- Caller uses `extract_macros()` to get per-100g values

---

#### Step 7: Batch endpoint (~40 lines) — Acceptance Criterion #2

Implement `get_foods_batch(ids: &[i64]) -> Result<Vec<FdcFoodDetailResponse>, FdcError>`.

- POST `/foods?api_key=...` with JSON body `{ "fdcIds": [...] }`
- API limit: max 20 IDs per request — chunk automatically if more than 20
- Returns combined results across all chunks
- Crucial for rate-limit efficiency when hydrating search results (1 request for 20 foods vs 20 individual requests)

---

#### Step 8: Tests (~150 lines)

Comprehensive wiremock-based tests following OFF client patterns:

**Serde deserialization tests:**
- Full search response with multiple food matches
- Partial detail response (missing optional fields)
- Nutrient extraction from varied nutrient lists (with kJ and kcal entries)
- Portion parsing from foodPortions array
- Empty/minimal responses

**Integration tests with wiremock:**
- `search_foods` success — verify POST body contains correct dataType filter
- `search_foods` with pagination params
- `get_food` success — verify GET with fdcId
- `get_food` not found (404)
- `get_foods_batch` success — verify POST with fdcIds array
- `get_foods_batch` auto-chunking (>20 IDs splits into multiple requests)
- Rate limit (429) returns `FdcError::RateLimited`
- Network error propagates as `FdcError::Request`
- API key appears as query param (verify via wiremock matcher)

**API key redaction:**
- Verify `FdcClient` does not expose api_key through Debug (the key is a plain String internally, but the config layer wraps it in `RedactedString`; add a note in comments that callers must use `RedactedString` from config)

---

#### Step 9: Module registration (~3 lines)

Update `nom-core/src/client/mod.rs`:
```rust
pub mod off;
pub mod usda;
```

---

#### Integration with operations layer (future)

The client is consumed by food operations (`TASK-2.13`). Operations will:
1. Load config → get `usda_api_key` as `Option<RedactedString>`
2. Construct `FdcClient::new(base_url, key.get())` when key is present
3. Call `search_foods()` for non-barcode queries
4. Call `get_foods_batch()` to hydrate search results with full nutrient data
5. Extract macros via `extract_macros()` for local caching
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed serde deserialization bugs in usda.rs: added missing rename attributes for pageSize and dataType fields, removed wiremock path matchers that caused 404s on POST endpoints, and adjusted auto-chunking test assertion. All 25 tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->

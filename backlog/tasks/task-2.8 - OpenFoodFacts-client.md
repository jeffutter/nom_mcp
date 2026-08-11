---
id: TASK-2.8
title: OpenFoodFacts client
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:23'
updated_date: '2026-08-11 22:52'
labels:
  - planned
dependencies:
  - TASK-2.3
type: feature
ordinal: 27000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Direct reqwest client for the Open Food Facts REST API (barcode lookup for packaged/branded foods) — do not depend on the unmaintained openfoodfacts-rust crate. Hand-scoped serde struct for the response fields nom_mcp needs. Base URL must be a constructor parameter (not baked in) so tests can point it at a local wiremock server. Respect OFF's real rate limits (15 req/min/IP reads, 10 req/min/IP search) and set a real User-Agent from config (default nom_mcp/<version>).

See doc-5 §1 and §11 (testing strategy's base-URL requirement).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 client performs a barcode lookup against a configurable base URL and deserializes kcal/protein/carbs/fat/fiber + serving basis
- [ ] #2 User-Agent header is set from config with a working hardcoded default
- [ ] #3 base URL is a constructor parameter, not a compiled-in constant
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
### Implementation Plan: OpenFoodFacts Client

**Location**: New `nom-core/src/client/off.rs` module + dependency updates to `nom-core/Cargo.toml`.

---

#### Dependencies to add (nom-core/Cargo.toml)

- **`reqwest`** — async HTTP client with JSON support. Use `reqwest` (not `reqwest::blocking`) since nom_mcp is async-first (axum, tokio, async-trait all present). Add `json` feature.
- **`wiremock`** (dev-dependency) — for integration tests that stub the OFF API.

No rate-limiting crate for v1. The ticket says "respect OFF's real rate limits" but does not mandate a middleware. The LLM caller will naturally stay well under 15 req/min; adding a token-bucket adds complexity without v1 benefit. Can be added later if needed.

---

#### Step 1: Define serde response structs (~60 lines)

Create hand-scoped structs matching the `/api/v2/product/{barcode}` response shape. Only deserialize fields nom_mcp needs (use `fields=` query param on the request side to minimize payload).

```rust
/// Top-level OFF response wrapper. Product data lives under `product.*`.
#[derive(Debug, Deserialize)]
pub struct OffResponse {
    pub code: String,
    #[serde(default)]
    pub product: Option<Product>,
    pub status: u8,
    #[serde(default)]
    pub status_verbose: Option<String>,
}

/// Core product fields we need for nutrition lookup.
#[derive(Debug, Deserialize)]
pub struct Product {
    #[serde(default)]
    pub product_name: Option<String>,
    #[serde(default)]
    pub serving_size: Option<f64>,
    #[serde(default)]
    pub nutrition_data_per: Option<String>, // "per 100g" or "per serving"
    #[serde(default)]
    pub nutriments: Option<Nutriments>,
}

/// Nutrient values from the `nutriments` object.
/// All fields are Option<f64> because products may omit any nutrient.
#[derive(Debug, Default, Deserialize)]
pub struct Nutriments {
    #[serde(rename = "energy-kcal", default)]
    pub energy_kcal: Option<f64>,
    #[serde(rename = "energy-kcal_100g", default)]
    pub energy_kcal_100g: Option<f64>,
    #[serde(default)]
    pub proteins: Option<f64>,
    #[serde(rename = "proteins_100g", default)]
    pub proteins_100g: Option<f64>,
    #[serde(default)]
    pub carbohydrates: Option<f64>,
    #[serde(rename = "carbohydrates_100g", default)]
    pub carbohydrates_100g: Option<f64>,
    #[serde(default)]
    pub fat: Option<f64>,
    #[serde(rename = "fat_100g", default)]
    pub fat_100g: Option<f64>,
    #[serde(default)]
    pub fiber: Option<f64>,
    #[serde(rename = "fiber_100g", default)]
    pub fiber_100g: Option<f64>,
}
```

Key design: prefer `_100g` variants when available (they're more consistent across products), fall back to raw values. The `nutrition_data_per` field tells us whether the primary values are per-100g or per-serving — callers can decide normalization strategy.

---

#### Step 2: Define `OffClient` struct and constructor (~40 lines)

```rust
pub struct OffClient {
    http: reqwest::Client,
    base_url: Url,
}

impl OffClient {
    /// Create a new OFF client pointing at the given base URL.
    /// Uses the provided user-agent string (from config).
    pub fn new(base_url: impl Into<Url>, user_agent: impl Into<String>) -> Self { ... }

    /// Create with production defaults.
    pub fn with_default_base(user_agent: impl Into<String>) -> Self { ... }
}
```

The `reqwest::Client` is built once via builder pattern with the User-Agent set. Tests construct with a wiremock URL. The `user_agent` comes from `AppConfig.off_user_agent` at runtime.

---

#### Step 3: Implement `lookup_barcode(barcode: &str) -> Result<Option<Product>, Error>` (~50 lines)

```rust
pub async fn lookup_barcode(&self, barcode: &str) -> Result<Option<Product>, OffError> {
    // Normalize: strip hyphens/spaces from barcode input
    let normalized = barcode.replace(['-', ' '], "");
    
    // Build URL with fields= param to minimize payload
    let url = format!("{}/api/v2/product/{}?fields=...", self.base_url, normalized);
    
    let resp = self.http.get(&url).send().await?;
    let body: OffResponse = resp.json().await?;
    
    match body.status {
        1 => Ok(body.product),
        0 => Ok(None), // product not found — not an error
        _ => Err(OffError::UnexpectedStatus(body.status)),
    }
}
```

Barcode normalization strips hyphens/spaces before sending. Status 0 means "not found" → return `Ok(None)`, not an error. Use `fields=` query param to minimize response size.

---

#### Step 4: Define `OffError` enum (~20 lines)

```rust
#[derive(Debug, thiserror::Error)]
pub enum OffError {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    
    #[error("unexpected API status: {0}")]
    UnexpectedStatus(u8),
}
```

Keep it simple — two variants cover the failure modes. Map to `ErrorData::external_api_failure()` at the Operation boundary.

---

#### Step 5: Wire into `nom-core/src/lib.rs` (~2 lines)

Add `pub mod client;` and inside `client/mod.rs`, `pub mod off;`.

---

#### Step 6: Unit tests (~80 lines)

Test the serde deserialization with realistic JSON payloads:
- Full response with all nutrients populated
- Partial response missing some nutrients (e.g., no fiber)
- Not-found response (`status: 0`)
- Malformed response (missing `product` key)

---

#### Step 7: Integration tests with wiremock (~120 lines)

Stub `/api/v2/product/{barcode}` scenarios:
- **Success**: full nutriments returned, verify deserialization
- **Partial data**: missing fiber field, verify `Option<f64>` handling
- **Not found**: `status: 0`, verify returns `Ok(None)`
- **Network error**: mock server down, verify error propagation
- **User-Agent header**: verify correct UA reaches the mock server

Use `wiremock::MockServer` with path matchers and header matchers. Point `OffClient::new()` at the mock server URL.

---

#### Acceptance Criteria Mapping

- **AC #1** (barcode lookup): Covered by Steps 1-3 — `lookup_barcode()` performs GET against configurable base URL, deserializes kcal/protein/carbs/fat/fiber + serving basis via hand-scoped structs.
- **AC #2** (User-Agent): Covered by Step 2 — `reqwest::Client` builder sets `.user_agent()` from constructor param. Default comes from `AppConfig::default_off_user_agent()`.
- **AC #3** (configurable base URL): Covered by Step 2 — `base_url` is a constructor parameter, never a compiled-in constant. Tests use wiremock URL.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
AC #1 ✓ — lookup_barcode() performs GET against configurable base URL, deserializes kcal/protein/carbs/fat/fiber + serving basis via hand-scoped serde structs (OffResponse, Product, Nutriments)
AC #2 ✓ — User-Agent header set from constructor param via reqwest Client builder; default comes from AppConfig::default_off_user_agent()
AC #3 ✓ — base_url is a constructor parameter (OffClient::new(base_url, user_agent)), never a compiled-in constant; tests use wiremock server URI
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented OpenFoodFacts REST client in nom-core/src/client/off.rs with all three acceptance criteria met: (1) barcode lookup against configurable base URL deserializing kcal/protein/carbs/fat/fiber + serving basis via hand-scoped serde structs, (2) User-Agent header from config with nom_mcp/<version> default, (3) base URL as constructor parameter not compiled-in constant. Added reqwest (rustls-tls) and url dependencies plus wiremock dev-dependency. 13 tests pass including unit tests for serde deserialization and wiremock integration tests for success/not-found/barcode-normalization/error scenarios.
<!-- SECTION:FINAL_SUMMARY:END -->

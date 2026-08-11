---
id: doc-3
title: 'Research: openfoodfacts-rust crate'
type: other
created_date: '2026-08-11 04:46'
updated_date: '2026-08-11 04:47'
---
## Summary / Build-buy-avoid signal

`openfoodfacts-rust` is a thin, low-level HTTP wrapper around the Open Food Facts (OFF) REST API. It is hosted under the official `openfoodfacts` GitHub org, but is functionally stalled: no feature or bugfix commits since March 2022 (aside from one Copilot lint pass), all recent activity is Dependabot CI bumps, it has never been published to crates.io (open issue since Jan 2022), has a real bug in its write-auth code, and an outside contributor's own open PR describes the repo as "an unmaintained repository." It provides no typed response models — every call returns the raw, undeserialized `reqwest` HTTP response, so the caller still writes their own `Product`/`Nutriments` deserialization regardless of whether the crate is used. **Recommendation: avoid as a dependency; call the OFF REST API directly with `reqwest` + a hand-written `serde` struct scoped to the fields nom_mcp needs.** The crate saves only URL-templating boilerplate (~15 lines) and brings a git-only, low-maintenance dependency in exchange.

---

## 1. Barcode lookup API surface

The crate exposes `OffClient::product()`:

```rust
pub fn product(&self, barcode: &str, output: Option<Output>) -> Result
```
where `Result = std::result::Result<reqwest::blocking::Response, Box<dyn std::error::Error>>` (`src/client.rs` lines 254-270, `src/lib.rs` lines 14-17).

It issues `GET https://{locale}.openfoodfacts.org/api/{v0|v2}/product/{barcode}` and returns the **raw, undeserialized HTTP response** — there is no typed `Product` struct anywhere in the crate. The caller must deserialize it themselves, e.g. per the README's own example:

```rust
let client = off::v0().build().unwrap();
let response = client.product("3850102123681", None).unwrap();
let result_json = json!(response.json::<HashMap<String, Value>>().unwrap());
```

Confirmed by reading `src/types.rs` in full: it contains only `V0`/`V2` API-version marker types and a `Params` type alias — no `Product`/`Nutriments` structs exist in the codebase.

Source: [src/client.rs](https://raw.githubusercontent.com/openfoodfacts/openfoodfacts-rust/develop/src/client.rs), [src/lib.rs](https://raw.githubusercontent.com/openfoodfacts/openfoodfacts-rust/develop/src/lib.rs), [src/types.rs](https://raw.githubusercontent.com/openfoodfacts/openfoodfacts-rust/develop/src/types.rs), [README](https://raw.githubusercontent.com/openfoodfacts/openfoodfacts-rust/develop/README.md).

## 2. Response shape / nutrition fields

Because the crate hands back raw JSON, the actual response shape is 100% defined by OFF's own API, not the crate. Verified empirically against the live API (`GET https://world.openfoodfacts.org/api/v2/product/3017620422003.json`, a Nutella product): the `product.nutriments` object contains (among others) `energy-kcal`, `energy-kj`, `fat`, `saturated-fat`, `carbohydrates`, `sugars`, `added-sugars`, `proteins`, `salt`, `sodium` — each typically present in `_100g` / `_serving` / `_unit` / `_value` suffixed variants. `fiber` is part of OFF's nutrient taxonomy but was **absent** on this particular product because the manufacturer/contributor never entered it — OFF nutrition data is crowd-submitted, so field coverage varies per product and cannot be guaranteed for any given barcode.

So: every macro nom_mcp needs (calories, protein, carbs, fat, fiber, sugar, sodium) exists in OFF's data model, but per-product completeness is not guaranteed and must be handled as optional/nullable in nom_mcp's own types regardless of whether this crate or raw HTTP is used.

Source: live query against [world.openfoodfacts.org/api/v2/product/3017620422003.json](https://world.openfoodfacts.org/api/v2/product/3017620422003.json); field/taxonomy background from [OFF API docs](https://openfoodfacts.github.io/openfoodfacts-server/api/).

## 3. Auth / User-Agent requirements

**The crate does set a User-Agent automatically**, contrary to an initial pass over the docs which suggested otherwise — verified directly in `src/lib.rs` lines 16, 130-134, 172-183:

```rust
user_agent: Some(format!(
    "OffRustClient - {} - Version {} - {}",
    OS, VERSION, "https://github.com/openfoodfacts/openfoodfacts-rust"
)),
```

This generic default is applied via `reqwest::ClientBuilder::user_agent()` in `build_http_client()` (`src/lib.rs` lines 186-207) whenever the caller doesn't override it. Callers *can* override it with `OffBuilder::user_agent("...")`, but are **not required to**.

This matters because OFF's own API policy (verified at [openfoodfacts.github.io/openfoodfacts-server/api/](https://openfoodfacts.github.io/openfoodfacts-server/api/)) requires a UA that identifies the *calling application*, in the format `AppName/Version (contact@example.com)`, explicitly "to not risk being identified as a bot." The crate's generic default (`"OffRustClient - linux - Version alpha - https://github.com/..."`) technically satisfies "a UA is present" but does not identify nom_mcp as required by policy, and if many consumers of this crate all ship the same default UA unmodified, that shared UA string is a plausible target for a blanket rate-limit/ban. There is an open upstream issue, [#29 "Require a custom user agent to use the SDK"](https://github.com/openfoodfacts/openfoodfacts-rust/issues/29) (filed Aug 2023, still open), where OFF/crate maintainers themselves flag that the library should force callers to supply one rather than silently defaulting.

**Action for nom_mcp if this crate (or a hand-rolled client) is used:** always call `.user_agent("nom_mcp/<version> (<contact-email>)")` explicitly; never rely on the crate default.

**Auth:** Optional HTTP Basic-auth is supported only for *write* operations via `OffBuilder::auth(username, password)` — irrelevant for read-only barcode lookups, since "READ operations... do not require authentication other than the custom User-Agent" per OFF's docs. Note for completeness: the crate's Basic-auth implementation has a real bug — `format!("Basic {}:{}", user, pass)` (`src/lib.rs` line 191) is **not base64-encoded**, violating RFC 7617 / HTTP Basic Auth. This is called out and being fixed in an open, unmerged draft PR ([#69](https://github.com/openfoodfacts/openfoodfacts-rust/pull/69), opened May 2026). Not relevant to barcode-lookup-only usage, but is a data point on code quality.

## 4. Rate limits (from Open Food Facts' own API docs — the crate enforces nothing)

The crate implements **no client-side rate limiting, retry, or backoff** of any kind — confirmed by reading the full `src/client.rs`/`src/lib.rs` source; `get()` is a bare `reqwest` GET call with no throttling logic.

Per OFF's official API documentation ([openfoodfacts.github.io/openfoodfacts-server/api/](https://openfoodfacts.github.io/openfoodfacts-server/api/)):
- **15 requests/min/IP** for read product queries (`GET /api/v*/product/...` or product page)
- **10 requests/min/IP** for search queries (`GET /api/v*/search` or `GET /cgi/search.pl`)
- Exceeding limits risks an **IP ban** (reversible by emailing the OFF team); a global infrastructure-level limit returns **HTTP 503** if exceeded more broadly.
- If requests originate per-end-user (e.g., a client mobile app calling OFF directly), limits apply per user; nom_mcp, acting as a shared server-side integration, will hit these limits under its own IP and must implement its own request throttling/caching layer regardless of which HTTP client it uses.

## 5. Crate maturity

- **Not published to crates.io.** Verified directly against the crates.io API: `GET https://crates.io/api/v1/crates/openfoodfacts` → `{"errors":[{"detail":"crate \`openfoodfacts\` does not exist"}]}`; a name search also returns zero results. **Not on docs.rs either** (`https://docs.rs/crate/openfoodfacts` → HTTP 404). The README instructs consumers to add it as a **git dependency** (`openfoodfacts = { git = "https://github.com/openfoodfacts/openfoodfacts-rust.git" }`), not a crates.io version. This is a long-tracked, unresolved gap: [issue #9 "Automatic publication of the package"](https://github.com/openfoodfacts/openfoodfacts-rust/issues/9), filed January 2022, still open 4.5 years later.
- **Versioning is internally inconsistent.** One git tag exists, `v1.0.0`, but `Cargo.toml` on the `develop` branch still reads `version = "0.1.0"`, and `src/lib.rs` hardcodes `pub const VERSION: &str = "alpha"` (baked into the default User-Agent string). There's no coherent released-version story.
- **Org-hosted but not actively developed.** The repo lives under the official `openfoodfacts` GitHub org (created March 2020, not archived), and recent merges are approved by `teolemon`, a long-time OFF core maintainer — so it's not a random abandoned fork. However, essentially all commit activity visible in the last year (spanning June–Aug 2026, right up to the day before this research) is **Dependabot GitHub Actions version bumps**, merged with no code review beyond that. The last substantive change to the actual client logic (`src/client.rs`) was by the original author `jjdo` in **March 2022**, followed only by one Copilot-authored lint/formatting commit in September 2025. Top contributors by commit count: `teolemon` (91, almost entirely CI/merge commits) and `jjdo` (44, the original author, inactive on this repo since 2022).
- The README itself carries a `maintenance-status: passively-maintained` badge.
- There is a large, still-open **draft** PR ([#69](https://github.com/openfoodfacts/openfoodfacts-rust/pull/69), opened by an outside contributor in May 2026, last updated the same day) whose own description states it addresses issues in **"an unmaintained repository"** — fixing clippy warnings, the broken Basic-auth encoding, bumping the Rust edition, and adding missing `Cargo.toml` metadata (description/license/keywords) needed for a real crates.io publish. None of this has landed as of this research (Aug 2026).
- 13 open issues on the repo as of this research, several multi-year old (e.g. #9, #29 above).

**Net assessment:** officially OFF-org-adjacent and not abandoned in the archived sense, but maintenance is limited to automated dependency bumps; there has been no real feature or fix work landed in ~4 years, and it isn't installable from crates.io.

Sources: [GitHub repo](https://github.com/openfoodfacts/openfoodfacts-rust), [repo API metadata](https://api.github.com/repos/openfoodfacts/openfoodfacts-rust), [commit history](https://api.github.com/repos/openfoodfacts/openfoodfacts-rust/commits), [contributors](https://api.github.com/repos/openfoodfacts/openfoodfacts-rust/contributors), [issue #9](https://github.com/openfoodfacts/openfoodfacts-rust/issues/9), [issue #29](https://github.com/openfoodfacts/openfoodfacts-rust/issues/29), [PR #69](https://github.com/openfoodfacts/openfoodfacts-rust/pull/69), crates.io API (`crates.io/api/v1/crates/openfoodfacts`), docs.rs (`docs.rs/crate/openfoodfacts` → 404).

## 6. Beyond barcode lookup

Yes, the crate covers more than `product()`:
- **Search** via `SearchQueryV0` / `SearchQueryV2` builders — `.query().criteria(field, value, locale)`, plus `.ingredient()` and `.nutrient()` filters — hitting `GET /cgi/search.pl` (v0) or `GET /api/v2/search` (v2). This is brand/name/category/nutrient-criteria search over OFF's own (packaged/branded) catalog, not free-text search of arbitrary foods.
- **Batch barcode lookup** via `OffClient<V2>::products(barcodes: &str)` — comma-separated list of codes in one call.
- **Metadata endpoints**: `taxonomy()`, `facet()`, `categories()`, `nutrients()`, `products_by(facet_or_category, id)` — static/reference data (allergen lists, category trees, etc.), not product-specific.

(Source: `src/client.rs`, `src/search.rs`, README feature table.)

For nom_mcp's design (USDA FDC for whole/raw foods, OFF for packaged/branded foods): this doesn't change that division — OFF's database is inherently branded/packaged retail products, so its search feature is a name/brand search *within that same packaged-food scope*, not a generic food search that could replace USDA FDC for raw foods. It does mean that if nom_mcp ever wants "search branded products by name" as well as "look up by barcode," OFF's own search endpoints (used directly via `reqwest`, or via this crate) can serve both, without needing USDA FDC's branded-foods dataset for that purpose.

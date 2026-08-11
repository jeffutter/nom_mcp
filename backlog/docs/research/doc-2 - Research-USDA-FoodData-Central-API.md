---
id: doc-2
title: 'Research: USDA FoodData Central API'
type: other
created_date: '2026-08-11 04:45'
updated_date: '2026-08-11 04:46'
---
# Research: USDA FoodData Central API

## Bottom line

Use FDC for whole/raw foods by restricting `dataType` search results to `Foundation`, `SR Legacy`, and `Survey (FNDDS)` — filter out `Branded` since Open Food Facts already owns packaged/branded lookups (this is a supported, first-class search parameter, not a client-side workaround). Auth is a free, self-service api.data.gov key (name + email, essentially instant) with a default limit of 1,000 requests/hour/IP — plenty for a single-user server; the unauthenticated `DEMO_KEY` (30/hr, 50/day) is fine for dev but not for real use. Nutrient values in `foodNutrients` are reported per 100 g regardless of data type, with household/serving portion info available alongside (`foodPortions` for Foundation/SR Legacy/Survey, `servingSize`/`servingSizeUnit`/`householdServingFullText` for Branded). No usable Rust crate exists on crates.io or lib.rs — a small bespoke `reqwest`-based client is the right call.

---

## 1. Auth

- **Signup**: FDC API access requires "a data.gov API key" incorporated into every request; FDC's key-signup page (`https://fdc.nal.usda.gov/api-key-signup/`) embeds the standard api.data.gov signup form. [FDC API Key Signup](https://fdc.nal.usda.gov/api-key-signup/) / [FDC API Guide](https://fdc.nal.usda.gov/api-guide/)
- The api.data.gov signup form only asks for name + email; the key is generated and emailed essentially immediately (no approval workflow) — this is api.data.gov's standard, well-documented behavior across all agencies it fronts (FDC, NASA, SAM.gov, etc.). [api.data.gov Developer Manual](https://api.data.gov/docs/developer-manual/)
- **Rate limit, default/free key**: "Hourly Limit: 1,000 requests per hour" per API key/IP. Usage is exposed via `X-RateLimit-Limit` / `X-RateLimit-Remaining` response headers; exceeding the limit returns HTTP 429 and the block clears automatically after an hour. [api.data.gov Developer Manual](https://api.data.gov/docs/developer-manual/)
- **DEMO_KEY** (no signup needed, for exploration only): "Hourly Limit: 30 requests per IP address per hour" and "Daily Limit: 50 requests per IP address per day" — used for the live sample calls in this research. [api.data.gov Developer Manual](https://api.data.gov/docs/developer-manual/)
- Higher limits than the 1,000/hr default are available on request: "Contact FoodData Central if a higher request rate setting is needed." [FDC API Guide](https://fdc.nal.usda.gov/api-guide/)
- For nom_mcp (single user, low request volume) the default 1,000/hr key tier is comfortably sufficient; no need to request an elevated limit.

## 2. Search endpoint — `/fdc/v1/foods/search`

Supports both GET and POST. [FDC API Guide](https://fdc.nal.usda.gov/api-guide/) / [FDC OpenAPI spec](https://fdc.nal.usda.gov/api-spec/fdc_api.html)

**Request parameters** (from the OpenAPI spec, `https://fdc.nal.usda.gov/api-spec/fdc_api.html`, backed by the raw spec at `https://api.nal.usda.gov/fdc/v1/yaml-spec?api_key=DEMO_KEY`):
- `query` (required, string) — "One or more search terms. The string may include search operators."
- `dataType` (optional, array) — filter to one or more of `Branded`, `Foundation`, `Survey (FNDDS)`, `SR Legacy`.
- `pageSize` (optional, integer, default 50, min 1, max 200) — results per page.
- `pageNumber` (optional, integer) — "offset into the overall result set is expressed as (pageNumber * pageSize)."
- `sortBy` (optional, enum) — `dataType.keyword`, `lowercaseDescription.keyword`, `fdcId`, `publishedDate`.
- `sortOrder` (optional, enum) — `asc`, `desc`.
- `brandOwner` (optional, string) — "Filter results based on the brand owner of the food. Only applies to Branded Foods."
- `requireAllWords` (optional, boolean) — present per the Swagger UI parameter list on the spec page.

**Relevance/ranking**: the spec exposes a `score` field per result ("Relative score indicating how well the food matches search criteria") but the guide/spec do not document the underlying ranking algorithm (it's an Elasticsearch-backed relevance score — internals aren't published). [FDC OpenAPI spec](https://fdc.nal.usda.gov/api-spec/fdc_api.html)

**Response fields per result** (`SearchResultFood`): `fdcId`, `dataType`, `description` (required), `foodNutrients` (abridged nutrient array), `publicationDate`, `brandOwner` (Branded only), `gtinUpc` (Branded only), `ingredients` (Branded only, label ingredient list), `ndbNumber` (legacy foundation/SR ID), `score`. [FDC OpenAPI spec](https://fdc.nal.usda.gov/api-spec/fdc_api.html)

Confirmed live against a real call: `GET https://api.nal.usda.gov/fdc/v1/foods/search?query=apple&pageSize=1&api_key=DEMO_KEY` returned a Branded result with `servingSize: 154.0`, `servingSizeUnit: "g"`, and a `foodNutrients` array (Energy 52 KCAL, Protein 0 G, Fat 0.65 G, Carbohydrate 14.3 G, Fiber 3.2 G, Sugars 10.4 G, Calcium 0 MG, Iron 0.23 MG, Sodium 0 MG, Potassium 110 MG, Vitamin A 65 IU, Vitamin C 3.1 MG, Cholesterol 0 MG, saturated/trans fat 0 G). [live FDC API response](https://api.nal.usda.gov/fdc/v1/foods/search?query=apple&pageSize=1&api_key=DEMO_KEY)

## 3. Detail endpoints — `/fdc/v1/food/{fdcId}` and `/fdc/v1/foods`

[FDC OpenAPI spec](https://fdc.nal.usda.gov/api-spec/fdc_api.html)

- **`GET /fdc/v1/food/{fdcId}`**: path param `fdcId` (required); query params `format` (`abridged` | `full`, default `full`) and `nutrients` (up to 25 nutrient numbers to restrict the response to).
- **`GET/POST /fdc/v1/foods`**: batch variant. GET takes `fdcIds` (1–20, comma-separated or repeated param), `format`, `nutrients`. POST takes the same fields as a JSON body (`FoodsCriteria` schema: `fdcIds` int array 1–20, `format`, `nutrients` int array 1–25). Returns an array of food objects (typed per data type: Branded/Foundation/SR Legacy/Survey/Abridged).
- A further `foods/list` paging endpoint (to walk the whole database) is also part of the API per FDC's own field-guide documentation, though it wasn't a focus of this research. [FDC API notes via search](https://fdc.nal.usda.gov/api-guide/)

**Nutrient data shape** (`FoodNutrient`): `id`, `amount` (the value), `dataPoints`, `min`/`max`/`median` (Foundation Foods variability stats), nested `nutrient` object with `id`, `number` (USDA nutrient number), `name`, `unitName`. [FDC OpenAPI spec](https://fdc.nal.usda.gov/api-spec/fdc_api.html)

**Macros/nutrients present**: energy (KCAL), protein (G), total fat (G), carbohydrate by difference (G), fiber (G), total/added sugars (G), saturated/trans fat (G), sodium (MG), calcium (MG), iron (MG), potassium (MG), cholesterol (MG), vitamin A (IU), vitamin C (MG), plus (on Foundation Foods, which carry the deepest nutrient panels) items like water, ash, choline, vitamin B‑6 (MG), riboflavin (MG), vitamin B‑12 (µg, unit `UG`), and individual fatty acid breakdowns (e.g. `SFA 16:0`, `PUFA 18:2`). Confirmed live against `GET .../foods/search?query=chicken breast&dataType=Foundation&api_key=DEMO_KEY` (fdcId 2759004, "Lunchmeat, chicken breast, sliced"). [live FDC API response](https://api.nal.usda.gov/fdc/v1/foods/search?query=chicken%20breast&dataType=Foundation&pageSize=1&api_key=DEMO_KEY)

**Units**: `kcal` for energy, `g` for macros, `mg` for sodium/calcium/iron/potassium/cholesterol/B‑vitamins, `µg` (`UG`) for B‑12 and similar trace nutrients, `IU` for vitamin A — all as observed in the live responses above. [live FDC API responses]

**Serving basis — confirmed per 100 g**: `foodNutrients.amount` values are reported per 100 g of food regardless of data type. Evidence: the Branded "apple" result above reports `servingSize: 154 g` alongside `foodNutrients`, yet Energy = 52 kcal — which only matches raw apple's well-known ~52 kcal/100g value, not a 154 g serving (which would be ~80 kcal) — proving the `foodNutrients` array is per-100g even when a household `servingSize` is also present. The Foundation Foods chicken-breast result carries no `servingSize`/`servingSizeUnit` field at all, consistent with per-100g being the implicit universal basis. [live FDC API responses]
- **Household/serving portion**, alongside the per-100g values:
  - **Branded foods**: `servingSize` + `servingSizeUnit` (+ `householdServingFullText`, e.g. "1 ONZ") fields on the food record. [FDC OpenAPI spec](https://fdc.nal.usda.gov/api-spec/fdc_api.html)
  - **Foundation / SR Legacy / Survey foods**: a `foodPortions` array instead, each entry with `amount`, `gramWeight`, `portionDescription`, and a `measureUnit` object (unit name/abbreviation) — e.g. "1 cup, chopped" → gram weight. [FDC OpenAPI spec](https://fdc.nal.usda.gov/api-spec/fdc_api.html)

## 4. FDC data types

Per FDC's own data-type comparison page: [FDC Data Type Comparison](https://fdc.nal.usda.gov/data-documentation/)

| Data type | What it represents | Source / provenance | Update cadence | Quality character |
|---|---|---|---|---|
| **Foundation Foods** | Individual samples of commodity/minimally-processed foods, with variability metadata (sample count, location, date, analytical method, sometimes genotype/production practice) | USDA, analytically derived (lab-tested) | April & October each year | Highest precision; includes min/max/median stats per nutrient across samples |
| **SR Legacy** | Historic Standard Reference nutrient composition data (successor to the old USDA National Nutrient Database) | USDA — mix of lab analysis, calculation, and published literature | Final release April 2018 (frozen, no longer updated) | Broad legacy coverage, older/mixed-provenance values, no longer maintained |
| **Survey (FNDDS)** | Foods and beverages *as consumed*, with nutrients and portion weights, used in USDA's What We Eat In America / NHANES dietary survey | USDA, compiled/derived from other FDC data types (not independently analyzed) | Every 2 years, aligned with WWEIA/NHANES releases | Reflects consumed/prepared forms (e.g. "cooked, chicken breast, roasted") rather than raw commodities |
| **Branded** | Commercial packaged product data from the USDA Global Branded Foods Database | Manufacturers, from product label submissions (public-private partnership) | Monthly | Label-submitted, not independently verified; per-serving label values plus derived per-100g |
| **Experimental** | Data tied to peer-reviewed research involving USDA | Researchers, from scientific publications | April & October, as available | Narrow/research-specific coverage, not general-purpose |

[FDC Data Type Comparison](https://fdc.nal.usda.gov/data-documentation/), [FDC API Guide](https://fdc.nal.usda.gov/api-guide/)

**Recommendation — filter out Branded**: Yes, exclude `Branded` from FDC queries in nom_mcp by passing `dataType=["Foundation","SR Legacy","Survey (FNDDS)"]` on `/fdc/v1/foods/search`. Open Food Facts already serves the packaged/branded-food use case (barcode lookup, label-sourced data), so pulling Branded results from FDC too would just create duplicate, differently-shaped candidates for the same packaged products and complicate the resolution/merge logic in the domain layer. Restricting FDC to Foundation + SR Legacy + Survey keeps FDC squarely scoped to its complementary job: whole/raw commodities (Foundation/SR Legacy) and as-consumed/prepared foods (Survey/FNDDS) that Open Food Facts doesn't cover well. Experimental is narrow enough (research-only) that it's not worth including either.

## 5. Rust ecosystem

- **crates.io**: searching `fooddata` returns zero results; searching `usda` and `fdc` return no relevant crates at all (results are unrelated projects — a USDZ 3D-format reader, hydrology models, an `FDC1004` capacitance-sensor driver, `fdcan`/`fdcanusb` CAN-bus drivers, etc.). [crates.io search: fooddata](https://crates.io/api/v1/crates?q=fooddata), [crates.io search: usda](https://crates.io/api/v1/crates?q=usda), [crates.io search: fdc](https://crates.io/api/v1/crates?q=fdc)
- **lib.rs**: searching `fooddata` returns "Nothing found." [lib.rs search: fooddata](https://lib.rs/search?q=fooddata)
- Broader web search turns up FDC API clients in other languages (Python's `usda-fdc` on ReadTheDocs, a Ruby `usda_fdc` gem, a generated TypeScript client `usda-food-data-central-client` on GitHub) and one incidental Rust implementation buried inside an unrelated MCP server project (`pierre_mcp_server`'s `usda_client.rs`) rather than a standalone, published, reusable crate. [usda-fdc Python docs](https://usda-fdc.readthedocs.io/en/stable/), [zokioki/usda_fdc Ruby gem](https://github.com/zokioki/usda_fdc), [john-hix/usda-food-data-central-client](https://github.com/john-hix/usda-food-data-central-client)
- **Recommendation**: no usable Rust crate exists. Given the API surface nom_mcp actually needs is small (search + food-detail + batch-detail, all plain JSON over HTTPS with a query-string API key), a bespoke `reqwest`-based client with a handful of typed response structs (mirroring the OpenAPI schemas documented above) is the right call — comparable in scope to whatever thin wrapper this project will already write for Open Food Facts, and avoids taking on an unmaintained/single-purpose dependency for a small amount of code.

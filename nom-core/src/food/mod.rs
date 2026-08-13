//! Food operations — search, resolution, and custom food creation.
//!
//! Implements `search_food` and `create_custom_food` per doc-5 §5, §6.
//! Searching IS resolving: every returned candidate is upserted into the
//! local foods cache, carrying a `food_id` immediately usable by `log_meal`.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;

use crate::client::{off::OffClient, usda::FdcClient};
use crate::error::ErrorData;
use crate::operation::Operation;
use crate::storage::Connection;

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// Unified response item for food search results.
///
/// Carries `food_id`, `name`, `source`, and full nutrient snapshot so callers
/// can disambiguate and log meals without a separate detail lookup.
#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct FoodCandidate {
    /// Auto-incremented ID from the local foods table.
    pub food_id: i64,
    /// Display name.
    pub name: String,
    /// Data source: "OpenFoodFacts", "USDA_FDC", or "Custom".
    pub source: String,
    /// Calories per 100g.
    #[serde(rename = "calories_per_100g")]
    pub calories_per_100g: f64,
    /// Protein (g) per 100g.
    #[serde(rename = "protein_g_per_100g")]
    pub protein_g_per_100g: f64,
    /// Carbohydrates (g) per 100g.
    #[serde(rename = "carbs_g_per_100g")]
    pub carbs_g_per_100g: f64,
    /// Fat (g) per 100g.
    #[serde(rename = "fat_g_per_100g")]
    pub fat_g_per_100g: f64,
    /// Fiber (g) per 100g.
    #[serde(rename = "fiber_g_per_100g")]
    pub fiber_g_per_100g: f64,
    /// Serving size in grams (nullable).
    #[serde(rename = "serving_size_g", skip_serializing_if = "Option::is_none")]
    pub serving_size_g: Option<f64>,
}

/// Grouped nutrient values per 100g.
///
/// Used as a single parameter for DB helper functions to avoid
/// `too_many_arguments` clippy warnings and transposition hazards.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NutrientValues {
    pub(crate) calories: f64,
    pub(crate) protein_g: f64,
    pub(crate) carbs_g: f64,
    pub(crate) fat_g: f64,
    pub(crate) fiber_g: f64,
}

/// Convert per-serving nutrients to per-100g values.
///
/// Formula: `(nutrient_at_serving * 100.0) / serving_size_g`
/// If `serving_size_g == 0.0`, returns the raw value (stored as-is with warning).
fn convert_to_per_100g(nutrient_value: f64, serving_size_g: f64) -> f64 {
    if serving_size_g <= 0.0 {
        tracing::warn!(
            serving_size_g,
            "zero or negative serving size; storing raw nutrient value"
        );
        nutrient_value
    } else {
        (nutrient_value * 100.0) / serving_size_g
    }
}

/// Detect whether a query string is barcode-shaped (all ASCII digits, non-empty).
fn is_barcode(query: &str) -> bool {
    !query.is_empty() && query.chars().all(|c| c.is_ascii_digit())
}

// ---------------------------------------------------------------------------
// DB helpers
// ---------------------------------------------------------------------------

/// Upsert a catalog food (OpenFoodFacts or USDA_FDC) into the foods table.
///
/// Uses `INSERT ... ON CONFLICT(source, external_id) DO UPDATE` to ensure
/// idempotency — repeated searches do not duplicate rows.
/// Returns the `id` of the (possibly existing) row.
async fn upsert_catalog_food(
    conn: &Connection,
    source: &str,
    external_id: &str,
    name: &str,
    nutrients: NutrientValues,
    serving_size_g: Option<f64>,
) -> Result<i64, ErrorData> {
    let sql = r#"
        INSERT INTO foods (source, external_id, name, calories_per_100g, protein_g_per_100g,
                           carbs_g_per_100g, fat_g_per_100g, fiber_g_per_100g, serving_size_g)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(source, external_id) DO UPDATE SET
            name = EXCLUDED.name,
            calories_per_100g = EXCLUDED.calories_per_100g,
            protein_g_per_100g = EXCLUDED.protein_g_per_100g,
            carbs_g_per_100g = EXCLUDED.carbs_g_per_100g,
            fat_g_per_100g = EXCLUDED.fat_g_per_100g,
            fiber_g_per_100g = EXCLUDED.fiber_g_per_100g,
            serving_size_g = EXCLUDED.serving_size_g
        RETURNING id
    "#;

    let mut stmt = conn
        .prepare(sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare upsert failed: {e}")))?;

    let mut rows = stmt
        .query((
            source,
            external_id,
            name,
            nutrients.calories,
            nutrients.protein_g,
            nutrients.carbs_g,
            nutrients.fat_g,
            nutrients.fiber_g,
            serving_size_g,
        ))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("upsert query failed: {e}")))?;

    if let Some(row) = rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("failed to read upsert result: {e}")))?
    {
        let value = row
            .get_value(0)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read food_id: {e}")))?;
        match value {
            turso::Value::Integer(id) => Ok(id),
            other => Err(ErrorData::storage_failure(format!(
                "unexpected value type for food_id: {:?}",
                other
            ))),
        }
    } else {
        Err(ErrorData::storage_failure(
            "upsert returned no row".to_string(),
        ))
    }
}

/// Insert a new Custom food into the foods table.
///
/// No conflict handling needed since `external_id` is NULL and there's no
/// unique constraint on just `source` for Custom entries.
async fn insert_custom_food(
    conn: &Connection,
    name: &str,
    nutrients: NutrientValues,
    serving_size_g: Option<f64>,
) -> Result<i64, ErrorData> {
    let sql = r#"
        INSERT INTO foods (source, external_id, name, calories_per_100g, protein_g_per_100g,
                           carbs_g_per_100g, fat_g_per_100g, fiber_g_per_100g, serving_size_g)
        VALUES ('Custom', NULL, ?, ?, ?, ?, ?, ?, ?)
        RETURNING id
    "#;

    let mut stmt = conn
        .prepare(sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare insert failed: {e}")))?;

    let mut rows = stmt
        .query((
            name,
            nutrients.calories,
            nutrients.protein_g,
            nutrients.carbs_g,
            nutrients.fat_g,
            nutrients.fiber_g,
            serving_size_g,
        ))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("insert query failed: {e}")))?;

    if let Some(row) = rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("failed to read insert result: {e}")))?
    {
        let value = row
            .get_value(0)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read food_id: {e}")))?;
        match value {
            turso::Value::Integer(id) => Ok(id),
            other => Err(ErrorData::storage_failure(format!(
                "unexpected value type for food_id: {:?}",
                other
            ))),
        }
    } else {
        Err(ErrorData::storage_failure(
            "insert returned no row".to_string(),
        ))
    }
}

/// Search Custom Foods by case-insensitive substring match.
async fn search_custom_foods(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<FoodCandidate>, ErrorData> {
    let like_pattern = format!("%{}%", query.to_lowercase());
    let sql = r#"
        SELECT id, name, calories_per_100g, protein_g_per_100g, carbs_g_per_100g,
               fat_g_per_100g, fiber_g_per_100g, serving_size_g
        FROM foods
        WHERE source = 'Custom' AND LOWER(name) LIKE ?
        LIMIT ?
    "#;

    let mut stmt = conn
        .prepare(sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare search failed: {e}")))?;

    let mut rows = stmt
        .query((&like_pattern[..], limit as i64))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("search query failed: {e}")))?;

    let mut results = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("failed to read row: {e}")))?
    {
        let id = row
            .get_value(0)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read food_id: {e}")))?;
        let id = match id {
            turso::Value::Integer(v) => v,
            _ => return Err(ErrorData::storage_failure("invalid food_id type")),
        };
        let name = row
            .get::<String>(1)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read name: {e}")))?;
        let calories = row
            .get::<f64>(2)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read calories: {e}")))?;
        let protein = row
            .get::<f64>(3)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read protein: {e}")))?;
        let carbs = row
            .get::<f64>(4)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read carbs: {e}")))?;
        let fat = row
            .get::<f64>(5)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read fat: {e}")))?;
        let fiber = row
            .get::<f64>(6)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read fiber: {e}")))?;
        let serving_size_g = match row
            .get_value(7)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read serving_size: {e}")))?
        {
            turso::Value::Real(v) => Some(v),
            turso::Value::Null => None,
            other => {
                return Err(ErrorData::storage_failure(format!(
                    "unexpected value type for serving_size: {:?}",
                    other
                )));
            }
        };

        results.push(FoodCandidate {
            food_id: id,
            name,
            source: "Custom".to_string(),
            calories_per_100g: calories,
            protein_g_per_100g: protein,
            carbs_g_per_100g: carbs,
            fat_g_per_100g: fat,
            fiber_g_per_100g: fiber,
            serving_size_g,
        });
    }

    Ok(results)
}

/// Extract macros from an OFF Product, preferring `_100g` fields when available.
fn extract_off_macros(product: &crate::client::off::Product) -> NutrientValues {
    let nutriments = product.nutriments.as_ref();
    NutrientValues {
        calories: nutriments.and_then(|n| n.energy_kcal_100g).unwrap_or(0.0),
        protein_g: nutriments.and_then(|n| n.proteins_100g).unwrap_or(0.0),
        carbs_g: nutriments.and_then(|n| n.carbohydrates_100g).unwrap_or(0.0),
        fat_g: nutriments.and_then(|n| n.fat_100g).unwrap_or(0.0),
        fiber_g: nutriments.and_then(|n| n.fiber_100g).unwrap_or(0.0),
    }
}

/// Merge search results: Custom-first ordering, deduplicate by name (case-insensitive), cap at total.
///
/// Within each group (Custom, then USDA) the incoming order is preserved rather
/// than re-sorted, since callers pass candidates in relevance order (Custom
/// results from the DB match, USDA results in the search API's relevance
/// ranking). Re-sorting alphabetically here would bury the most relevant hit
/// under any candidate whose name happens to start earlier in the alphabet.
fn merge_candidates(mut candidates: Vec<FoodCandidate>, cap: usize) -> Vec<FoodCandidate> {
    // Stable sort on Custom-vs-not only — preserves relevance order within each group.
    candidates.sort_by_key(|c| c.source != "Custom");

    // Deduplicate by name (case-insensitive), keeping the first (most relevant) occurrence.
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.name.to_lowercase()));

    // Cap
    candidates.truncate(cap);
    candidates
}

// ---------------------------------------------------------------------------
// SearchFood Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchFoodRequest {
    /// Search query — barcode digits, free-text name, or dish description.
    pub query: String,
}

pub struct SearchFood {
    off_client: Arc<OffClient>,
    fdc_client: Option<Arc<FdcClient>>,
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl SearchFood {
    pub fn new(off_client: Arc<OffClient>, fdc_client: Option<Arc<FdcClient>>) -> Self {
        Self {
            off_client,
            fdc_client,
            #[cfg(test)]
            db_path: None,
        }
    }

    /// Set a custom database path (used by tests).
    #[cfg(test)]
    pub fn with_db_path(mut self, path: std::path::PathBuf) -> Self {
        self.db_path = Some(path);
        self
    }
}

#[async_trait::async_trait]
impl Operation for SearchFood {
    fn name(&self) -> &str {
        "search_food"
    }

    fn description(&self) -> &str {
        "Search for foods by barcode or name. Barcode queries (all digits) search OpenFoodFacts only. Free-text queries search custom foods and USDA FDC. Results are cached locally and include food_id for immediate use in log_meal. Search before creating custom foods to avoid duplicates."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(SearchFoodRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: SearchFoodRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("query", format!("invalid request: {e}")))?;

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path).await?
        } else {
            Connection::open().await?
        };

        #[cfg(not(test))]
        let conn = Connection::open().await?;

        let candidates = if is_barcode(&req.query) {
            // Barcode path: OpenFoodFacts only
            self.search_barcode(&req.query, &conn).await?
        } else {
            // Free-text path: Custom Foods + USDA FDC, merged
            self.search_free_text(&req.query, &conn).await?
        };

        Ok(serde_json::json!(candidates))
    }
}

impl SearchFood {
    async fn search_barcode(
        &self,
        query: &str,
        conn: &Connection,
    ) -> Result<Vec<FoodCandidate>, ErrorData> {
        match self.off_client.lookup_barcode(query).await {
            Err(e) => Err(ErrorData::external_api_failure(format!(
                "OpenFoodFacts lookup failed: {e}"
            ))),
            Ok(None) => Ok(Vec::new()),
            Ok(Some(product)) => {
                let name = product
                    .product_name
                    .clone()
                    .unwrap_or_else(|| query.to_string());
                let macros = extract_off_macros(&product);
                let serving_size_g = product.serving_size;

                // Use the barcode as external_id
                let external_id = query.replace(['-', ' ', '\t'], "");

                let food_id = upsert_catalog_food(
                    conn,
                    "OpenFoodFacts",
                    &external_id,
                    &name,
                    macros,
                    serving_size_g,
                )
                .await?;

                Ok(vec![FoodCandidate {
                    food_id,
                    name,
                    source: "OpenFoodFacts".to_string(),
                    calories_per_100g: macros.calories,
                    protein_g_per_100g: macros.protein_g,
                    carbs_g_per_100g: macros.carbs_g,
                    fat_g_per_100g: macros.fat_g,
                    fiber_g_per_100g: macros.fiber_g,
                    serving_size_g,
                }])
            }
        }
    }

    async fn search_free_text(
        &self,
        query: &str,
        conn: &Connection,
    ) -> Result<Vec<FoodCandidate>, ErrorData> {
        let mut all_candidates = Vec::new();

        // 1. Search Custom Foods (local DB)
        let custom_results = search_custom_foods(conn, query, 5).await?;
        all_candidates.extend(custom_results);

        // 2. Search USDA FDC (only if client is available)
        if let Some(fdc) = &self.fdc_client {
            match fdc.search_foods(query, 1).await {
                Ok(search_resp) => {
                    if !search_resp.food_matches.is_empty() {
                        // Only fetch details for a small buffer past the final cap
                        // (some may be deduped against Custom results below) — the
                        // search response is already in USDA's relevance order, so
                        // taking the head keeps the most relevant hits and avoids
                        // fetching full nutrient details for every one of the up to
                        // 50 matches on the page.
                        let ids: Vec<i64> = search_resp
                            .food_matches
                            .iter()
                            .take(10)
                            .map(|m| m.fdc_id)
                            .collect();

                        // Batch-fetch details and upsert each
                        match fdc.get_foods_batch(&ids).await {
                            Ok(mut foods) => {
                                // The batch endpoint doesn't guarantee it echoes
                                // results back in request order — restore USDA's
                                // relevance ranking before candidates are built.
                                let rank: std::collections::HashMap<i64, usize> =
                                    ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
                                foods.sort_by_key(|f| {
                                    rank.get(&f.fdc_id).copied().unwrap_or(usize::MAX)
                                });

                                for food in foods {
                                    let macros = food.extract_macros();
                                    let name = food.description.clone().unwrap_or_default();
                                    if name.is_empty() {
                                        continue;
                                    }

                                    let serving_size_g =
                                        food.portion_info().first().map(|p| p.gram_weight);

                                    let nutrients = NutrientValues {
                                        calories: macros.energy_kcal.unwrap_or(0.0),
                                        protein_g: macros.protein_g.unwrap_or(0.0),
                                        carbs_g: macros.carbs_g.unwrap_or(0.0),
                                        fat_g: macros.fat_g.unwrap_or(0.0),
                                        fiber_g: macros.fiber_g.unwrap_or(0.0),
                                    };

                                    let food_id = upsert_catalog_food(
                                        conn,
                                        "USDA_FDC",
                                        &food.fdc_id.to_string(),
                                        &name,
                                        nutrients,
                                        serving_size_g,
                                    )
                                    .await?;

                                    all_candidates.push(FoodCandidate {
                                        food_id,
                                        name,
                                        source: "USDA_FDC".to_string(),
                                        calories_per_100g: nutrients.calories,
                                        protein_g_per_100g: nutrients.protein_g,
                                        carbs_g_per_100g: nutrients.carbs_g,
                                        fat_g_per_100g: nutrients.fat_g,
                                        fiber_g_per_100g: nutrients.fiber_g,
                                        serving_size_g,
                                    });
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "USDA FDC batch fetch failed");
                                // Don't fail the entire search — Custom results still valid
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "USDA FDC search failed");
                    // Don't fail the entire search — Custom results still valid
                }
            }
        }
        // No USDA FDC client configured — return only Custom results

        // Merge, deduplicate, cap at 5
        Ok(merge_candidates(all_candidates, 5))
    }
}

// ---------------------------------------------------------------------------
// CreateCustomFood Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct CreateCustomFoodRequest {
    /// Name of the food/dish.
    pub name: String,
    /// Serving size specification.
    pub serving_size: ServingSize,
    /// Nutrient values for one serving.
    pub nutrients: Nutrients,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ServingSize {
    /// Numeric quantity.
    pub quantity: f64,
    /// Unit string. Only gram-equivalent units are accepted: "grams", "gram", or "g".
    pub unit: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct Nutrients {
    /// Calories in one serving.
    pub calories: f64,
    /// Protein (g) in one serving.
    pub protein_g: f64,
    /// Carbohydrates (g) in one serving.
    pub carbs_g: f64,
    /// Fat (g) in one serving.
    pub fat_g: f64,
    /// Fiber (g) in one serving.
    pub fiber_g: f64,
}

pub struct CreateCustomFood {
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl Default for CreateCustomFood {
    fn default() -> Self {
        Self::new()
    }
}

impl CreateCustomFood {
    pub fn new() -> Self {
        Self {
            #[cfg(test)]
            db_path: None,
        }
    }

    /// Set a custom database path (used by tests).
    #[cfg(test)]
    pub fn with_db_path(mut self, path: std::path::PathBuf) -> Self {
        self.db_path = Some(path);
        self
    }
}

#[async_trait::async_trait]
impl Operation for CreateCustomFood {
    fn name(&self) -> &str {
        "create_custom_food"
    }

    fn description(&self) -> &str {
        "Create a custom food entry with nutrition data. Nutrients are provided per one serving (not per 100g). serving_size.unit must be a gram-equivalent unit ('grams', 'gram', or 'g') — other units are rejected. Search before creating to avoid duplicates — reuse relies on search_food's substring match."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(CreateCustomFoodRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: CreateCustomFoodRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        // Validate serving size
        if req.serving_size.quantity <= 0.0 {
            return Err(ErrorData::validation(
                "serving_size.quantity",
                "must be greater than zero",
            ));
        }

        // Only gram-based units are accepted. Volume/piece units require
        // ingredient-specific density data that v1 has no source for.
        let unit_lower = req.serving_size.unit.to_lowercase();
        if unit_lower != "grams" && unit_lower != "gram" && unit_lower != "g" {
            return Err(ErrorData::validation(
                "serving_size.unit",
                format!(
                    "only gram-based units are supported (got '{}'); volume units like cups or pieces cannot be converted without ingredient-specific density data",
                    req.serving_size.unit
                ),
            ));
        }

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path).await?
        } else {
            Connection::open().await?
        };

        #[cfg(not(test))]
        let conn = Connection::open().await?;

        // Convert per-serving nutrients to per-100g
        // Unit is already validated as gram-based above
        let serving_size_g = req.serving_size.quantity;
        let nutrients = NutrientValues {
            calories: convert_to_per_100g(req.nutrients.calories, serving_size_g),
            protein_g: convert_to_per_100g(req.nutrients.protein_g, serving_size_g),
            carbs_g: convert_to_per_100g(req.nutrients.carbs_g, serving_size_g),
            fat_g: convert_to_per_100g(req.nutrients.fat_g, serving_size_g),
            fiber_g: convert_to_per_100g(req.nutrients.fiber_g, serving_size_g),
        };

        let food_id = insert_custom_food(&conn, &req.name, nutrients, Some(serving_size_g)).await?;

        Ok(serde_json::json!(FoodCandidate {
            food_id,
            name: req.name,
            source: "Custom".to_string(),
            calories_per_100g: nutrients.calories,
            protein_g_per_100g: nutrients.protein_g,
            carbs_g_per_100g: nutrients.carbs_g,
            fat_g_per_100g: nutrients.fat_g,
            fiber_g_per_100g: nutrients.fiber_g,
            serving_size_g: Some(serving_size_g),
        }))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test::TempDb;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    // -- Helper factories --

    fn make_off_client(base_url: &str) -> OffClient {
        OffClient::new(base_url, "test-agent/1.0").unwrap()
    }

    fn make_fdc_client(base_url: &str) -> FdcClient {
        FdcClient::new(base_url, "test-key").unwrap()
    }

    // -- Barcode detection --

    #[test]
    fn test_is_barcode_digit_only() {
        assert!(is_barcode("123456"));
        assert!(is_barcode("001234567890"));
    }

    #[test]
    fn test_is_barcode_rejects_non_digits() {
        assert!(!is_barcode("abc123"));
        assert!(!is_barcode("chicken breast"));
        assert!(!is_barcode("123-456"));
        assert!(!is_barcode("123 456"));
    }

    #[test]
    fn test_is_barcode_empty() {
        assert!(!is_barcode(""));
    }

    // -- Conversion helper --

    #[test]
    fn test_convert_to_per_100g_basic() {
        assert!((convert_to_per_100g(200.0, 100.0) - 200.0).abs() < 0.001);
        assert!((convert_to_per_100g(150.0, 50.0) - 300.0).abs() < 0.001);
        assert!((convert_to_per_100g(10.0, 25.0) - 40.0).abs() < 0.001);
    }

    #[test]
    fn test_convert_to_per_100g_zero_serving() {
        // Zero serving returns raw value
        assert_eq!(convert_to_per_100g(200.0, 0.0), 200.0);
    }

    // -- OFF macro extraction --

    #[test]
    fn test_extract_off_macros_prefers_100g_fields() {
        let product = crate::client::off::Product {
            product_name: Some("Test".into()),
            serving_size: Some(50.0),
            nutrition_data_per: Some("per serving".into()),
            nutriments: Some(crate::client::off::Nutriments {
                energy_kcal: Some(150.0),
                energy_kcal_100g: Some(300.0),
                proteins: Some(7.5),
                proteins_100g: Some(15.0),
                carbohydrates: Some(20.0),
                carbohydrates_100g: Some(40.0),
                fat: Some(2.5),
                fat_100g: Some(5.0),
                fiber: Some(1.5),
                fiber_100g: Some(3.0),
            }),
        };
        let macros = extract_off_macros(&product);
        assert_eq!(macros.calories, 300.0);
        assert_eq!(macros.protein_g, 15.0);
        assert_eq!(macros.carbs_g, 40.0);
        assert_eq!(macros.fat_g, 5.0);
        assert_eq!(macros.fiber_g, 3.0);
    }

    #[test]
    fn test_extract_off_macros_defaults_to_zero_when_missing() {
        let product = crate::client::off::Product {
            product_name: Some("Empty".into()),
            serving_size: None,
            nutrition_data_per: None,
            nutriments: Some(crate::client::off::Nutriments::default()),
        };
        let macros = extract_off_macros(&product);
        assert_eq!(macros.calories, 0.0);
        assert_eq!(macros.protein_g, 0.0);
        assert_eq!(macros.carbs_g, 0.0);
        assert_eq!(macros.fat_g, 0.0);
        assert_eq!(macros.fiber_g, 0.0);
    }

    #[test]
    fn test_extract_off_macros_no_nutriments() {
        let product = crate::client::off::Product {
            product_name: Some("No Nutriments".into()),
            serving_size: None,
            nutrition_data_per: None,
            nutriments: None,
        };
        let macros = extract_off_macros(&product);
        assert_eq!(macros.calories, 0.0);
        assert_eq!(macros.protein_g, 0.0);
        assert_eq!(macros.carbs_g, 0.0);
        assert_eq!(macros.fat_g, 0.0);
        assert_eq!(macros.fiber_g, 0.0);
    }

    // -- Merge/dedup logic --

    #[test]
    fn test_merge_candidates_custom_first_and_cap() {
        let mut cands = vec![
            FoodCandidate {
                food_id: 1,
                name: "Chicken".to_string(),
                source: "USDA_FDC".to_string(),
                calories_per_100g: 200.0,
                protein_g_per_100g: 30.0,
                carbs_g_per_100g: 0.0,
                fat_g_per_100g: 5.0,
                fiber_g_per_100g: 0.0,
                serving_size_g: None,
            },
            FoodCandidate {
                food_id: 2,
                name: "Chicken".to_string(),
                source: "Custom".to_string(),
                calories_per_100g: 250.0,
                protein_g_per_100g: 25.0,
                carbs_g_per_100g: 5.0,
                fat_g_per_100g: 10.0,
                fiber_g_per_100g: 0.0,
                serving_size_g: Some(100.0),
            },
            FoodCandidate {
                food_id: 3,
                name: "Rice".to_string(),
                source: "USDA_FDC".to_string(),
                calories_per_100g: 130.0,
                protein_g_per_100g: 2.0,
                carbs_g_per_100g: 28.0,
                fat_g_per_100g: 0.3,
                fiber_g_per_100g: 0.4,
                serving_size_g: None,
            },
        ];
        // Add more to test capping
        for i in 4..=10 {
            cands.push(FoodCandidate {
                food_id: i,
                name: format!("Item{}", i),
                source: "USDA_FDC".to_string(),
                calories_per_100g: 100.0,
                protein_g_per_100g: 10.0,
                carbs_g_per_100g: 10.0,
                fat_g_per_100g: 5.0,
                fiber_g_per_100g: 1.0,
                serving_size_g: None,
            });
        }

        let result = merge_candidates(cands, 5);
        // Custom "Chicken" should come first (custom-first sort)
        assert_eq!(result[0].source, "Custom");
        assert_eq!(result[0].name, "Chicken");
        // After dedup, "Chicken" appears once
        assert_eq!(result.iter().filter(|c| c.name == "Chicken").count(), 1);
        // Capped at 5
        assert_eq!(result.len(), 5);
    }

    // -- Integration tests with temp DB + wiremock --

    #[serial_test::serial]
    #[tokio::test]
    async fn test_search_food_barcode_success() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": "123456",
                "status": 1,
                "product": {
                    "product_name": "Widget",
                    "serving_size": 50.0,
                    "nutrition_data_per": "per serving",
                    "nutriments": {
                        "energy-kcal_100g": 300.0,
                        "proteins_100g": 12.0,
                        "carbohydrates_100g": 25.0,
                        "fat_100g": 5.0,
                        "fiber_100g": 3.0
                    }
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let db = TempDb::new().await;
        let off = Arc::new(make_off_client(&base_url));
        let fdc = Arc::new(make_fdc_client(&base_url));
        let op = SearchFood::new(off, Some(fdc)).with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({"query": "123456"})))
            .await
            .unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let item = &arr[0];
        assert_eq!(item["name"], "Widget");
        assert_eq!(item["source"], "OpenFoodFacts");
        assert_eq!(item["calories_per_100g"], 300.0);
        assert_eq!(item["protein_g_per_100g"], 12.0);
        assert!(item["food_id"].is_i64());
        assert!(item["food_id"].as_i64().unwrap() > 0);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_search_food_barcode_not_found() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": "000000000000",
                "status": 0
            })))
            .expect(1)
            .mount(&server)
            .await;

        let db = TempDb::new().await;
        let off = Arc::new(make_off_client(&base_url));
        let fdc = Arc::new(make_fdc_client(&base_url));
        let op = SearchFood::new(off, Some(fdc)).with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({"query": "000000000000"})))
            .await
            .unwrap();

        assert!(result.as_array().unwrap().is_empty());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_search_food_free_text_custom_only() {
        let db = TempDb::new().await;
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        // Pre-seed a custom food
        let conn = Connection::open_at(&db.path).await.unwrap();
        conn.execute(
            "INSERT INTO foods (source, name, calories_per_100g, protein_g_per_100g) \
             VALUES ('Custom', 'Homemade Chicken Salad', 250.0, 20.0)",
            (),
        )
        .await
        .unwrap();

        // USDA search returns empty. Real API returns matches under "foods",
        // not "foodMatches" — see FdcSearchResponse's doc comment.
        Mock::given(method("POST"))
            .and(path("/v1/foods/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "foods": [],
                "totalHits": 0
            })))
            .expect(1)
            .mount(&server)
            .await;

        let off = Arc::new(make_off_client(&base_url));
        let fdc = Arc::new(make_fdc_client(&base_url));
        let op = SearchFood::new(off, Some(fdc)).with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({"query": "chicken"})))
            .await
            .unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["source"], "Custom");
        assert_eq!(arr[0]["name"], "Homemade Chicken Salad");
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_search_food_free_text_usda_merge() {
        let db = TempDb::new().await;
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        // USDA search returns matches. Real API returns matches under "foods",
        // not "foodMatches" — see FdcSearchResponse's doc comment.
        Mock::given(method("POST"))
            .and(path("/v1/foods/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "foods": [
                    {"fdcId": 100000, "description": "Chicken Breast", "dataType": "Foundation"}
                ],
                "totalHits": 1
            })))
            .expect(1)
            .mount(&server)
            .await;

        // USDA batch detail. Real /v1/foods response is a bare array, not
        // wrapped in {"foods": [...]} — see get_foods_batch's doc comment.
        // nutrient.number is a string on the wire ("208"), not an int.
        Mock::given(method("POST"))
            .and(path("/v1/foods"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "fdcId": 100000,
                    "description": "Chicken Breast",
                    "dataType": "Foundation",
                    "foodNutrients": [
                        {"nutrient": {"number": "208", "name": "Energy", "unitName": "kcal"}, "amount": 231.0},
                        {"nutrient": {"number": "203", "name": "Protein", "unitName": "g"}, "amount": 31.0},
                        {"nutrient": {"number": "204", "name": "Total lipid (fat)", "unitName": "g"}, "amount": 5.0},
                        {"nutrient": {"number": "205", "name": "Carbohydrate, by difference", "unitName": "g"}, "amount": 0.0},
                        {"nutrient": {"number": "291", "name": "Fiber, total dietary", "unitName": "g"}, "amount": 0.0}
                    ],
                    "foodPortions": [
                        {"modifier": "", "gramWeight": 140.0, "portionDescription": "1 breast", "amount": 140.0}
                    ]
                }])),
            )
            .expect(1)
            .mount(&server)
            .await;

        let off = Arc::new(make_off_client(&base_url));
        let fdc = Arc::new(make_fdc_client(&base_url));
        let op = SearchFood::new(off, Some(fdc)).with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({"query": "chicken"})))
            .await
            .unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["source"], "USDA_FDC");
        assert_eq!(arr[0]["name"], "Chicken Breast");
        assert_eq!(arr[0]["calories_per_100g"], 231.0);
        assert_eq!(arr[0]["protein_g_per_100g"], 31.0);
        assert!(arr[0]["food_id"].is_i64());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_search_food_upsert_idempotency() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": "123456",
                "status": 1,
                "product": {
                    "product_name": "Widget",
                    "nutriments": {"energy-kcal_100g": 300.0}
                }
            })))
            .mount(&server)
            .await;

        let db = TempDb::new().await;
        let off = Arc::new(make_off_client(&base_url));
        let fdc = Arc::new(make_fdc_client(&base_url));
        let op = SearchFood::new(off, Some(fdc)).with_db_path(db.path.clone());

        // First call
        let r1 = op
            .execute_json(Arc::new(serde_json::json!({"query": "123456"})))
            .await
            .unwrap();
        let id1 = r1.as_array().unwrap()[0]["food_id"].as_i64().unwrap();

        // Second call with same barcode
        let r2 = op
            .execute_json(Arc::new(serde_json::json!({"query": "123456"})))
            .await
            .unwrap();
        let id2 = r2.as_array().unwrap()[0]["food_id"].as_i64().unwrap();

        // Same food_id — upsert did not create a duplicate
        assert_eq!(id1, id2);

        // Verify only one row exists
        let conn = Connection::open_at(&db.path).await.unwrap();
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM foods WHERE source='OpenFoodFacts'")
            .await
            .unwrap();
        let mut rows = stmt.query(()).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let count = row.get::<i64>(0).unwrap();
        assert_eq!(count, 1);
    }

    // -- CreateCustomFood tests --

    #[serial_test::serial]
    #[tokio::test]
    async fn test_create_custom_food_stores_per_100g() {
        let db = TempDb::new().await;

        let op = CreateCustomFood::new().with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "name": "Homemade Pasta",
                "serving_size": {"quantity": 200.0, "unit": "grams"},
                "nutrients": {
                    "calories": 400.0,
                    "protein_g": 15.0,
                    "carbs_g": 60.0,
                    "fat_g": 8.0,
                    "fiber_g": 3.0
                }
            })))
            .await
            .unwrap();

        // Verify per-100g conversion: 400 cal / 200g * 100 = 200 cal/100g
        assert_eq!(result["calories_per_100g"], 200.0);
        assert_eq!(result["protein_g_per_100g"], 7.5);
        assert_eq!(result["carbs_g_per_100g"], 30.0);
        assert_eq!(result["fat_g_per_100g"], 4.0);
        assert_eq!(result["fiber_g_per_100g"], 1.5);
        assert_eq!(result["source"], "Custom");
        assert_eq!(result["serving_size_g"], 200.0);
        assert!(result["food_id"].is_i64());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_create_custom_food_rejects_non_gram_unit() {
        let db = TempDb::new().await;

        let op = CreateCustomFood::new().with_db_path(db.path.clone());
        let err = op
            .execute_json(Arc::new(serde_json::json!({
                "name": "Cup of Soup",
                "serving_size": {"quantity": 1.0, "unit": "cups"},
                "nutrients": {
                    "calories": 100.0,
                    "protein_g": 5.0,
                    "carbs_g": 15.0,
                    "fat_g": 2.0,
                    "fiber_g": 1.0
                }
            })))
            .await
            .unwrap_err();

        assert_eq!(err.category, crate::error::ErrorCategory::Validation);
        assert_eq!(err.field.as_deref(), Some("serving_size.unit"));
        assert!(
            err.reason
                .as_ref()
                .map(|r| r.contains("cups"))
                .unwrap_or(false)
        );
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_create_custom_food_accepts_gram_aliases() {
        let db = TempDb::new().await;

        for unit_alias in ["grams", "gram", "g"] {
            let op = CreateCustomFood::new().with_db_path(db.path.clone());
            let result = op
                .execute_json(Arc::new(serde_json::json!({
                    "name": format!("Test {}", unit_alias),
                    "serving_size": {"quantity": 200.0, "unit": unit_alias},
                    "nutrients": {
                        "calories": 400.0,
                        "protein_g": 15.0,
                        "carbs_g": 60.0,
                        "fat_g": 8.0,
                        "fiber_g": 3.0
                    }
                })))
                .await
                .unwrap();

            // Verify per-100g conversion: 400 cal / 200g * 100 = 200 cal/100g
            assert_eq!(result["calories_per_100g"], 200.0);
            assert_eq!(result["serving_size_g"], 200.0);
        }
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_create_custom_food_rejects_zero_serving() {
        let db = TempDb::new().await;

        let op = CreateCustomFood::new().with_db_path(db.path.clone());
        let err = op
            .execute_json(Arc::new(serde_json::json!({
                "name": "Bad Food",
                "serving_size": {"quantity": 0.0, "unit": "grams"},
                "nutrients": {"calories": 100.0, "protein_g": 0.0, "carbs_g": 0.0, "fat_g": 0.0, "fiber_g": 0.0}
            })))
            .await
            .unwrap_err();

        assert_eq!(err.category, crate::error::ErrorCategory::Validation);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_create_custom_food_rejects_negative_serving() {
        let db = TempDb::new().await;

        let op = CreateCustomFood::new().with_db_path(db.path.clone());
        let err = op
            .execute_json(Arc::new(serde_json::json!({
                "name": "Bad Food",
                "serving_size": {"quantity": -1.0, "unit": "grams"},
                "nutrients": {"calories": 100.0, "protein_g": 0.0, "carbs_g": 0.0, "fat_g": 0.0, "fiber_g": 0.0}
            })))
            .await
            .unwrap_err();

        assert_eq!(err.category, crate::error::ErrorCategory::Validation);
    }

    // -- Prove parsed CLI params flow through to an Operation --

    /// Proves that parse_params() output actually drives SearchFood behavior,
    /// not just parse_params()'s own return value.
    #[serial_test::serial]
    #[tokio::test]
    async fn test_search_food_via_parsed_cli_params_proves_params_flow_to_operation() {
        let db = TempDb::new().await;
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        // Pre-seed a custom food
        let conn = Connection::open_at(&db.path).await.unwrap();
        conn.execute(
            "INSERT INTO foods (source, name, calories_per_100g, protein_g_per_100g) \
             VALUES ('Custom', 'Almond Butter', 610.0, 21.0)",
            (),
        )
        .await
        .unwrap();

        // USDA search returns empty. Real API returns matches under "foods",
        // not "foodMatches" — see FdcSearchResponse's doc comment.
        Mock::given(method("POST"))
            .and(path("/v1/foods/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "foods": [],
                "totalHits": 0
            })))
            .expect(1)
            .mount(&server)
            .await;

        let off = Arc::new(make_off_client(&base_url));
        let fdc = Arc::new(make_fdc_client(&base_url));

        // --- Part A: with parsed CLI params, operation finds the seeded food ---
        let params_with_query = crate::cli::parse_params(&["query=almond".into()]).unwrap();
        let op_a = SearchFood::new(off.clone(), Some(fdc.clone())).with_db_path(db.path.clone());
        let result = op_a
            .execute_json(Arc::new(params_with_query))
            .await
            .unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["source"], "Custom");
        assert_eq!(arr[0]["name"], "Almond Butter");

        // --- Part B: without params, operation fails because query is required ---
        let params_empty = crate::cli::parse_params(&[]).unwrap();
        let op_b = SearchFood::new(off, Some(fdc)).with_db_path(db.path.clone());
        let err = op_b.execute_json(Arc::new(params_empty)).await.unwrap_err();

        assert_eq!(err.category, crate::error::ErrorCategory::Validation);
        assert_eq!(err.field.as_deref(), Some("query"));
    }
}

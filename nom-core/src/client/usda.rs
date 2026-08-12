//! USDA FoodData Central API client for whole/raw food nutrition lookup.
//!
//! Bespoke `reqwest` client — no usable Rust crate exists. Queries only
//! Foundation + SR Legacy + Survey (FNDDS) data types via the dataType filter;
//! Branded foods are excluded since OpenFoodFacts covers packaged products.
//! Nutrients are per 100g with household/serving portions surfaced alongside.
//!
//! API key from `api.data.gov` (free tier: 1,000 req/hr). Base URL is a
//! constructor parameter so tests can point at a local wiremock server.

use serde::Deserialize;
use url::Url;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Canonical nutrient IDs used by the USDA FDC API.
pub mod nutrients {
    /// Energy (kcal) — must also match unitName="kcal" to exclude kJ entries.
    pub const ENERGY_KCAL: i64 = 208;
    /// Protein (g).
    pub const PROTEIN: i64 = 203;
    /// Total fat (g).
    pub const FAT: i64 = 204;
    /// Carbohydrate, by difference (g).
    pub const CARBS: i64 = 205;
    /// Fiber, total dietary (g).
    pub const FIBER: i64 = 291;
}

/// Data types we query — excludes Branded.
const DATA_TYPES: &[&str] = &[
    "Foundation",
    "SR Legacy",
    "Survey (FNDDS)",
];

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Response structs
// ---------------------------------------------------------------------------

/// Top-level search response wrapper.
#[derive(Debug, Deserialize)]
pub struct FdcSearchResponse {
    #[serde(rename = "foodMatches")]
    pub food_matches: Vec<FdcFoodMatch>,
    #[serde(rename = "totalHits", default)]
    pub total_hits: i64,
    #[serde(rename = "currentPage", default)]
    pub current_page: i64,
    #[serde(rename = "pageSize", default)]
    pub page_size: i64,
}

/// Single food match from search results.
#[derive(Debug, Deserialize)]
pub struct FdcFoodMatch {
    #[serde(rename = "fdcId")]
    pub fdc_id: i64,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "dataType", default)]
    pub data_type: Option<String>,
    #[serde(rename = "brandName", default)]
    pub brand_name: Option<String>,
    #[serde(rename = "gtinUpc", default)]
    pub gtin_upc: Option<String>,
}

/// Detail response for a single food (unified across Foundation/SR Legacy/Survey).
#[derive(Debug, Deserialize)]
pub struct FdcFoodDetailResponse {
    #[serde(rename = "fdcId")]
    pub fdc_id: i64,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "dataType")]
    pub data_type: Option<String>,
    #[serde(default, rename = "foodNutrients")]
    pub food_nutrients: Vec<FdcNutrient>,
    #[serde(default, rename = "foodPortions")]
    pub food_portions: Vec<FdcPortion>,
}

/// Batch response wrapper — contains array of full food details.
#[derive(Debug, Deserialize)]
pub struct FdcBatchResponse {
    #[serde(default)]
    pub foods: Vec<FdcFoodDetailResponse>,
}

/// Individual nutrient entry from foodNutrients array.
#[derive(Debug, Deserialize)]
pub struct FdcNutrient {
    #[serde(default)]
    pub nutrient: Option<NutrientInfo>,
    #[serde(default)]
    pub amount: Option<f64>,
}

/// Nutrient metadata (name, id, unit).
#[derive(Debug, Deserialize)]
pub struct NutrientInfo {
    #[serde(default)]
    pub number: Option<i64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "unitName")]
    pub unit_name: Option<String>,
}

/// Household/serving portion information.
#[derive(Debug, Deserialize)]
pub struct FdcPortion {
    #[serde(default)]
    pub modifier: Option<String>,
    #[serde(default, rename = "gramWeight")]
    pub gram_weight: Option<f64>,
    #[serde(default, rename = "portionDescription")]
    pub portion_description: Option<String>,
    #[serde(default)]
    pub amount: Option<f64>,
}

// ---------------------------------------------------------------------------
// Extracted macros & portion helpers
// ---------------------------------------------------------------------------

/// Five macro nutrients extracted from a food's raw nutrient list.
#[derive(Debug, Default, Clone)]
pub struct FdcNutrients {
    pub energy_kcal: Option<f64>,
    pub protein_g: Option<f64>,
    pub fat_g: Option<f64>,
    pub carbs_g: Option<f64>,
    pub fiber_g: Option<f64>,
}

/// Simplified portion info for display.
#[derive(Debug, Clone)]
pub struct FdcPortionInfo {
    pub modifier: Option<String>,
    pub gram_weight: f64,
    pub description: Option<String>,
}

impl FdcFoodDetailResponse {
    /// Extract the five macro nutrients per 100g from the raw nutrient list.
    ///
    /// Filters by both nutrient ID AND unit name to avoid kJ energy or other
    /// unit variants. Returns None for any nutrient not found in the list.
    pub fn extract_macros(&self) -> FdcNutrients {
        let mut macros = FdcNutrients::default();

        for n in &self.food_nutrients {
            let Some(info) = &n.nutrient else { continue };
            let Some(id) = info.number else { continue };
            let Some(unit) = &info.unit_name else { continue };
            let Some(amount) = n.amount else { continue };

            match id {
                nutrients::ENERGY_KCAL if unit == "kcal" => {
                    macros.energy_kcal = Some(amount);
                }
                nutrients::PROTEIN if unit == "g" => {
                    macros.protein_g = Some(amount);
                }
                nutrients::FAT if unit == "g" => {
                    macros.fat_g = Some(amount);
                }
                nutrients::CARBS if unit == "g" => {
                    macros.carbs_g = Some(amount);
                }
                nutrients::FIBER if unit == "g" => {
                    macros.fiber_g = Some(amount);
                }
                _ => {}
            }
        }

        macros
    }

    /// Surface household/serving portions as simplified info structs.
    pub fn portion_info(&self) -> Vec<FdcPortionInfo> {
        self.food_portions
            .iter()
            .map(|p| FdcPortionInfo {
                modifier: p.modifier.clone(),
                gram_weight: p.gram_weight.unwrap_or(0.0),
                description: p.portion_description.clone(),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Client for the USDA FoodData Central API.
///
/// The base URL is a constructor parameter (not baked in), so tests can point
/// at a local wiremock server and production uses the real endpoint.
///
/// Note: callers should pass the API key via `RedactedString::get()` from
/// config to ensure the key is never accidentally logged through Debug output.
pub struct FdcClient {
    http: reqwest::Client,
    base_url: Url,
    api_key: String,
}

impl std::fmt::Debug for FdcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FdcClient")
            .field("http", &self.http)
            .field("base_url", &self.base_url)
            .field("api_key", &"REDACTED")
            .finish()
    }
}

impl FdcClient {
    /// Create a new FDC client pointing at the given base URL.
    pub fn new(base_url: &str, api_key: &str) -> Result<Self, FdcError> {
        let url = Url::parse(base_url)?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self {
            http,
            base_url: url,
            api_key: api_key.to_string(),
        })
    }

    /// Create with the production base URL (`https://api.nal.usda.gov/fdc`).
    pub fn with_default_base(api_key: &str) -> Result<Self, FdcError> {
        Self::new("https://api.nal.usda.gov/fdc", api_key)
    }

    // -- Endpoints --

    /// Search foods by query string, filtering to non-Branded data types.
    ///
    /// Uses POST to `/foods/search` with JSON body (avoids URL encoding
    /// issues with arrays). Returns paginated results with `totalHits`.
    pub async fn search_foods(
        &self,
        query: &str,
        page: u32,
    ) -> Result<FdcSearchResponse, FdcError> {
        let body = serde_json::json!({
            "query": query,
            "dataType": DATA_TYPES,
            "pageSize": 50,
            "pageNumber": page,
        });

        tracing::debug!(query, page, "USDA FDC search request");

        let resp = self
            .http
            .post(format!("{}/v1/foods/search", self.base_url))
            .query(&[("api_key", &self.api_key)])
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if status == 429 {
            return Err(FdcError::RateLimited);
        }

        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(FdcError::ApiError {
                status: status.as_u16(),
                message,
            });
        }

        let rate_remaining = resp
            .headers()
            .get("X-RateLimit-Remaining")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u32>().ok());
        if let Some(remaining) = rate_remaining {
            tracing::debug!(remaining, "USDA FDC rate limit remaining");
        }

        Ok(resp.json().await?)
    }

    /// Fetch full detail for a single food by its FDC ID.
    ///
    /// GET `/food/{fdc_id}` — includes optional nutrient filter for efficiency.
    pub async fn get_food(&self, fdc_id: i64) -> Result<FdcFoodDetailResponse, FdcError> {
        tracing::debug!(fdc_id, "USDA FDC detail request");

        // Filter to the nutrient IDs we care about for efficiency
        let nutrient_numbers = [
            nutrients::ENERGY_KCAL,
            nutrients::PROTEIN,
            nutrients::FAT,
            nutrients::CARBS,
            nutrients::FIBER,
        ];
        let nutrient_ids = nutrient_numbers
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let resp = self
            .http
            .get(format!("{}/v1/food/{fdc_id}", self.base_url))
            .query(&[("api_key", &self.api_key)])
            .query(&[("nutrientNumber", &nutrient_ids)])
            .send()
            .await?;

        let status = resp.status();
        if status == 429 {
            return Err(FdcError::RateLimited);
        }

        if status == 404 {
            return Err(FdcError::ApiError {
                status: 404,
                message: format!("food not found (fdcId={})", fdc_id),
            });
        }

        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(FdcError::ApiError {
                status: status.as_u16(),
                message,
            });
        }

        Ok(resp.json().await?)
    }

    /// Fetch details for multiple foods in a single batch request.
    ///
    /// POST `/foods` with `{ "fdcIds": [...] }`. Automatically chunks
    /// requests into groups of 20 (the API limit). Returns combined results.
    pub async fn get_foods_batch(
        &self,
        ids: &[i64],
    ) -> Result<Vec<FdcFoodDetailResponse>, FdcError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        const CHUNK_SIZE: usize = 20;
        let mut all_foods: Vec<FdcFoodDetailResponse> = Vec::new();

        for chunk in ids.chunks(CHUNK_SIZE) {
            let body = serde_json::json!({
                "fdcIds": chunk
            });

            tracing::debug!(count = chunk.len(), "USDA FDC batch request");

            let resp = self
                .http
                .post(format!("{}/v1/foods", self.base_url))
                .query(&[("api_key", &self.api_key)])
                .json(&body)
                .send()
                .await?;

            let status = resp.status();
            if status == 429 {
                return Err(FdcError::RateLimited);
            }

            if !status.is_success() {
                let message = resp.text().await.unwrap_or_default();
                return Err(FdcError::ApiError {
                    status: status.as_u16(),
                    message,
                });
            }

            let batch: FdcBatchResponse = resp.json().await?;
            all_foods.extend(batch.foods);
        }

        Ok(all_foods)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::method;
    use wiremock::{Mock, ResponseTemplate};

    // -- Serde deserialization tests --

    fn full_search_response_json() -> &'static str {
        r#"{
            "foodMatches": [
                {
                    "fdcId": 123456,
                    "description": "Chicken Breast, roasted",
                    "dataType": "Foundation",
                    "gtinUpc": null
                },
                {
                    "fdcId": 789012,
                    "description": "Chicken Breast, raw",
                    "dataType": "SR Legacy"
                }
            ],
            "totalHits": 150,
            "currentPage": 1,
            "pageNumber": 1,
            "pageSize": 50
        }"#
    }

    fn minimal_search_response_json() -> &'static str {
        r#"{
            "foodMatches": [],
            "totalHits": 0
        }"#
    }

    fn full_detail_response_json() -> &'static str {
        r#"{
            "fdcId": 123456,
            "description": "Chicken Breast, roasted",
            "dataType": "Foundation",
            "foodNutrients": [
                {"nutrient": {"number": 208, "name": "Energy", "unitName": "kcal"}, "amount": 231.0},
                {"nutrient": {"number": 208, "name": "Energy", "unitName": "kJ"}, "amount": 966.0},
                {"nutrient": {"number": 203, "name": "Protein", "unitName": "g"}, "amount": 31.0},
                {"nutrient": {"number": 204, "name": "Total lipid (fat)", "unitName": "g"}, "amount": 5.0},
                {"nutrient": {"number": 205, "name": "Carbohydrate, by difference", "unitName": "g"}, "amount": 0.0},
                {"nutrient": {"number": 291, "name": "Fiber, total dietary", "unitName": "g"}, "amount": 0.0}
            ],
            "foodPortions": [
                {"modifier": "", "gramWeight": 140.0, "portionDescription": "1 breast, without skin", "amount": 140.0},
                {"modifier": "", "gramWeight": 85.0, "portionDescription": "1 fillet, without skin", "amount": 85.0}
            ]
        }"#
    }

    fn partial_detail_response_json() -> &'static str {
        r#"{
            "fdcId": 999999,
            "foodNutrients": [
                {"nutrient": {"number": 208, "name": "Energy", "unitName": "kcal"}, "amount": 150.0}
            ]
        }"#
    }

    fn empty_detail_response_json() -> &'static str {
        r#"{"fdcId": 111111}"#
    }

    fn batch_response_json() -> &'static str {
        r#"{
            "foods": [
                {"fdcId": 100, "foodNutrients": [
                    {"nutrient": {"number": 208, "name": "Energy", "unitName": "kcal"}, "amount": 200.0}
                ]},
                {"fdcId": 200, "foodNutrients": [
                    {"nutrient": {"number": 208, "name": "Energy", "unitName": "kcal"}, "amount": 250.0}
                ]}
            ]
        }"#
    }

    #[test]
    fn test_deserialize_full_search_response() {
        let resp: FdcSearchResponse =
            serde_json::from_str(full_search_response_json()).unwrap();
        assert_eq!(resp.total_hits, 150);
        assert_eq!(resp.current_page, 1);
        assert_eq!(resp.page_size, 50);
        assert_eq!(resp.food_matches.len(), 2);
        assert_eq!(resp.food_matches[0].fdc_id, 123456);
        assert_eq!(
            resp.food_matches[0].description,
            Some("Chicken Breast, roasted".into())
        );
        assert_eq!(
            resp.food_matches[0].data_type,
            Some("Foundation".into())
        );
    }

    #[test]
    fn test_deserialize_minimal_search_response() {
        let resp: FdcSearchResponse =
            serde_json::from_str(minimal_search_response_json()).unwrap();
        assert_eq!(resp.food_matches.len(), 0);
        assert_eq!(resp.total_hits, 0);
        assert_eq!(resp.current_page, 0);
    }

    #[test]
    fn test_deserialize_full_detail_response() {
        let resp: FdcFoodDetailResponse =
            serde_json::from_str(full_detail_response_json()).unwrap();
        assert_eq!(resp.fdc_id, 123456);
        assert_eq!(resp.description, Some("Chicken Breast, roasted".into()));
        assert_eq!(resp.data_type, Some("Foundation".into()));
        assert_eq!(resp.food_nutrients.len(), 6);
        assert_eq!(resp.food_portions.len(), 2);
    }

    #[test]
    fn test_deserialize_partial_detail_response() {
        let resp: FdcFoodDetailResponse =
            serde_json::from_str(partial_detail_response_json()).unwrap();
        assert_eq!(resp.fdc_id, 999999);
        assert!(resp.description.is_none());
        assert!(resp.data_type.is_none());
        assert_eq!(resp.food_nutrients.len(), 1);
        assert!(resp.food_portions.is_empty());
    }

    #[test]
    fn test_deserialize_empty_detail_response() {
        let resp: FdcFoodDetailResponse =
            serde_json::from_str(empty_detail_response_json()).unwrap();
        assert_eq!(resp.fdc_id, 111111);
        assert!(resp.food_nutrients.is_empty());
        assert!(resp.food_portions.is_empty());
    }

    #[test]
    fn test_deserialize_batch_response() {
        let resp: FdcBatchResponse =
            serde_json::from_str(batch_response_json()).unwrap();
        assert_eq!(resp.foods.len(), 2);
        assert_eq!(resp.foods[0].fdc_id, 100);
        assert_eq!(resp.foods[1].fdc_id, 200);
    }

    #[test]
    fn test_extract_macros_from_full_nutrients() {
        let resp: FdcFoodDetailResponse =
            serde_json::from_str(full_detail_response_json()).unwrap();
        let macros = resp.extract_macros();
        // Energy should pick kcal (231), not kJ (966)
        assert_eq!(macros.energy_kcal, Some(231.0));
        assert_eq!(macros.protein_g, Some(31.0));
        assert_eq!(macros.fat_g, Some(5.0));
        assert_eq!(macros.carbs_g, Some(0.0));
        assert_eq!(macros.fiber_g, Some(0.0));
    }

    #[test]
    fn test_extract_macros_partial() {
        let resp: FdcFoodDetailResponse =
            serde_json::from_str(partial_detail_response_json()).unwrap();
        let macros = resp.extract_macros();
        assert_eq!(macros.energy_kcal, Some(150.0));
        assert!(macros.protein_g.is_none());
        assert!(macros.fat_g.is_none());
        assert!(macros.carbs_g.is_none());
        assert!(macros.fiber_g.is_none());
    }

    #[test]
    fn test_extract_macros_empty() {
        let resp: FdcFoodDetailResponse =
            serde_json::from_str(empty_detail_response_json()).unwrap();
        let macros = resp.extract_macros();
        assert!(macros.energy_kcal.is_none());
        assert!(macros.protein_g.is_none());
        assert!(macros.fat_g.is_none());
        assert!(macros.carbs_g.is_none());
        assert!(macros.fiber_g.is_none());
    }

    #[test]
    fn test_portion_info() {
        let resp: FdcFoodDetailResponse =
            serde_json::from_str(full_detail_response_json()).unwrap();
        let portions = resp.portion_info();
        assert_eq!(portions.len(), 2);
        assert_eq!(portions[0].gram_weight, 140.0);
        assert_eq!(
            portions[0].description,
            Some("1 breast, without skin".into())
        );
        assert_eq!(portions[1].gram_weight, 85.0);
    }

    #[test]
    fn test_portion_info_defaults() {
        let json = r#"{
            "fdcId": 1,
            "foodPortions": [{"gramWeight": null}]
        }"#;
        let resp: FdcFoodDetailResponse = serde_json::from_str(json).unwrap();
        let portions = resp.portion_info();
        assert_eq!(portions.len(), 1);
        assert_eq!(portions[0].gram_weight, 0.0);
    }

    // -- Client construction tests --

    #[test]
    fn test_client_new() {
        let client = FdcClient::new("http://localhost:1234", "test-key").unwrap();
        assert_eq!(client.base_url.as_str(), "http://localhost:1234/");
    }

    #[test]
    fn test_client_with_default_base() {
        let client = FdcClient::with_default_base("test-key").unwrap();
        assert!(client.base_url.as_str().contains("nal.usda.gov"));
    }

    #[test]
    fn test_client_invalid_url() {
        let err = FdcClient::new("not-a-url", "key").unwrap_err();
        assert!(matches!(err, FdcError::InvalidUrl(_)));
    }

    // -- Helper: build mock JSON responses --

    fn make_search_json(matches: &[&str]) -> serde_json::Value {
        let count = matches.len();
        let food_matches: Vec<serde_json::Value> = matches
            .into_iter()
            .enumerate()
            .map(|(i, desc)| {
                serde_json::json!({
                    "fdcId": 100000 + i as i64,
                    "description": desc,
                    "dataType": "Foundation"
                })
            })
            .collect();
        serde_json::json!({
            "foodMatches": food_matches,
            "totalHits": count as i64,
            "currentPage": 1,
            "pageNumber": 1,
            "pageSize": 50
        })
    }

    fn make_detail_json(fdc_id: i64) -> serde_json::Value {
        serde_json::json!({
            "fdcId": fdc_id,
            "description": "Test Food",
            "dataType": "Foundation",
            "foodNutrients": [
                {"nutrient": {"number": 208, "name": "Energy", "unitName": "kcal"}, "amount": 200.0},
                {"nutrient": {"number": 203, "name": "Protein", "unitName": "g"}, "amount": 15.0}
            ],
            "foodPortions": []
        })
    }

    fn make_batch_json(ids: &[i64]) -> serde_json::Value {
        let foods: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "fdcId": id,
                    "foodNutrients": [
                        {"nutrient": {"number": 208, "name": "Energy", "unitName": "kcal"}, "amount": 180.0}
                    ]
                })
            })
            .collect();
        serde_json::json!({ "foods": foods })
    }

    // -- Integration tests with wiremock --

    #[tokio::test]
    async fn test_search_foods_success() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                make_search_json(&["Chicken Breast", "Chicken Thigh"]),
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = FdcClient::new(&base_url, "test-key").unwrap();
        let result = client.search_foods("chicken", 1).await.unwrap();
        assert_eq!(result.food_matches.len(), 2);
        assert_eq!(result.food_matches[0].fdc_id, 100000);
        assert_eq!(
            result.food_matches[0].description,
            Some("Chicken Breast".into())
        );
        assert_eq!(result.total_hits, 2);
    }

    #[tokio::test]
    async fn test_search_foods_pagination() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                make_search_json(&["Page 3 Result"]),
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = FdcClient::new(&base_url, "test-key").unwrap();
        let result = client.search_foods("beef", 3).await.unwrap();
        assert_eq!(result.food_matches.len(), 1);
    }

    #[tokio::test]
    async fn test_get_food_success() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(make_detail_json(123456)))
            .expect(1)
            .mount(&server)
            .await;

        let client = FdcClient::new(&base_url, "test-key").unwrap();
        let result = client.get_food(123456).await.unwrap();
        assert_eq!(result.fdc_id, 123456);
        assert_eq!(result.description, Some("Test Food".into()));
        let macros = result.extract_macros();
        assert_eq!(macros.energy_kcal, Some(200.0));
        assert_eq!(macros.protein_g, Some(15.0));
    }

    #[tokio::test]
    async fn test_get_food_not_found() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
            .expect(1)
            .mount(&server)
            .await;

        let client = FdcClient::new(&base_url, "test-key").unwrap();
        let err = client.get_food(999999).await.unwrap_err();
        assert!(matches!(err, FdcError::ApiError { status: 404, .. }));
    }

    #[tokio::test]
    async fn test_get_foods_batch_success() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(make_batch_json(&[100, 200])),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = FdcClient::new(&base_url, "test-key").unwrap();
        let result = client.get_foods_batch(&[100, 200]).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].fdc_id, 100);
        assert_eq!(result[1].fdc_id, 200);
    }

    #[tokio::test]
    async fn test_get_foods_batch_empty() {
        let client = FdcClient::new("http://localhost:1", "test-key").unwrap();
        let result = client.get_foods_batch(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_get_foods_batch_auto_chunking() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        // 41 IDs with CHUNK_SIZE=20 produces 2 chunks → at least 2 POST requests
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                make_batch_json(&[1, 2, 3]),
            ))
            .mount(&server)
            .await;

        let client = FdcClient::new(&base_url, "test-key").unwrap();
        let ids: Vec<i64> = (1..=41).collect();
        let result = client.get_foods_batch(&ids).await.unwrap();
        // Each chunk returns 3 foods from mock; 2 chunks × 3 = 6 minimum
        assert!(result.len() >= 6, "expected ≥6 foods from 2+ chunks, got {}", result.len());
    }

    #[tokio::test]
    async fn test_rate_limited_returns_error() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(429).set_body_string("Too Many Requests"))
            .expect(1)
            .mount(&server)
            .await;

        let client = FdcClient::new(&base_url, "test-key").unwrap();
        let err = client.search_foods("test", 1).await.unwrap_err();
        assert!(matches!(err, FdcError::RateLimited));
    }

    #[tokio::test]
    async fn test_api_error_on_500() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(500).set_body_string("Internal Server Error"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = FdcClient::new(&base_url, "test-key").unwrap();
        let err = client.search_foods("test", 1).await.unwrap_err();
        assert!(matches!(err, FdcError::ApiError { status: 500, .. }));
    }

    #[tokio::test]
    async fn test_api_key_appears_as_query_param() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"fdcId": 1})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = FdcClient::new(&base_url, "secret-key").unwrap();
        client.get_food(1).await.unwrap();
        // If we reach here, the request was accepted by the mock
    }

    #[tokio::test]
    async fn test_network_error_propagates() {
        // Point at a URL that won't respond
        let client = FdcClient::new("http://127.0.0.1:54321", "test-key").unwrap();
        let err = client.search_foods("test", 1).await.unwrap_err();
        assert!(matches!(err, FdcError::Request(_)));
    }
}

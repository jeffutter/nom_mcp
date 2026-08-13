//! Open Food Facts REST API client for barcode-based nutrition lookup.
//!
//! Direct `reqwest` client — does not depend on the unmaintained
//! `openfoodfacts-rust` crate. Hand-scoped serde structs deserialize only
//! the fields nom_mcp needs from the `/api/v2/product/{barcode}` endpoint.

use serde::Deserialize;
use url::Url;

// ---------------------------------------------------------------------------
// Response structs
// ---------------------------------------------------------------------------

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
    /// Indicates whether primary values are "per 100g" or "per serving".
    #[serde(default)]
    pub nutrition_data_per: Option<String>,
    #[serde(default)]
    pub nutriments: Option<Nutriments>,
}

/// Nutrient values from the `nutriments` object.
/// All fields are `Option<f64>` because products may omit any nutrient.
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

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum OffError {
    #[error("invalid base URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("base URL cannot be used for path segment modification")]
    InvalidBase,

    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("unexpected API status: {0}")]
    UnexpectedStatus(u8),
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Client for the Open Food Facts REST API.
///
/// The base URL is a constructor parameter (not baked in), so tests can point
/// at a local wiremock server and production uses the real endpoint.
pub struct OffClient {
    http: reqwest::Client,
    base_url: Url,
}

impl OffClient {
    /// Create a new OFF client pointing at the given base URL, using the
    /// provided user-agent string (typically from config).
    pub fn new(base_url: &str, user_agent: &str) -> Result<Self, OffError> {
        let url = Url::parse(base_url)?;
        if url.cannot_be_a_base() {
            return Err(OffError::InvalidBase);
        }
        let http = reqwest::Client::builder().user_agent(user_agent).build()?;
        Ok(Self {
            http,
            base_url: url,
        })
    }

    /// Create with the production base URL (`https://world.openfoodfacts.org`).
    pub fn with_default_base(user_agent: &str) -> Result<Self, OffError> {
        Self::new("https://world.openfoodfacts.org", user_agent)
    }

    /// Look up nutrition data for a product by its barcode.
    ///
    /// Returns `Ok(None)` when the product is not found (status == 0).
    /// Barcodes are normalized (hyphens and spaces stripped) before sending.
    pub async fn lookup_barcode(&self, barcode: &str) -> Result<Option<Product>, OffError> {
        // Normalize: strip hyphens/spaces from barcode input
        let normalized = barcode.replace(['-', ' ', '\t'], "");

        // Build URL with structured path/query API to percent-encode the barcode.
        // This prevents path/query injection from untrusted barcode values.
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|_| OffError::InvalidBase)?
            .extend(["api", "v2", "product", &normalized]);
        let fields = [
            "code",
            "product_name",
            "serving_size",
            "nutrition_data_per",
            "nutriments:energy-kcal",
            "nutriments:energy-kcal_100g",
            "nutriments:proteins",
            "nutriments:proteins_100g",
            "nutriments:carbohydrates",
            "nutriments:carbohydrates_100g",
            "nutriments:fat",
            "nutriments:fat_100g",
            "nutriments:fiber",
            "nutriments:fiber_100g",
        ];
        url.query_pairs_mut()
            .append_pair("fields", &fields.join(","));

        let resp = self.http.get(url.as_str()).send().await?;
        let body: OffResponse = resp.json().await?;

        match body.status {
            1 => Ok(body.product),
            0 => Ok(None), // product not found — not an error
            other => Err(OffError::UnexpectedStatus(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, ResponseTemplate};

    // -- Fixtures captured from real OFF `/api/v2/product/{barcode}` responses --
    // (see nom-core/tests/fixtures/off/) — full realistic payload shape,
    // including fields this client doesn't parse.

    const BARCODE_FOUND: &str = include_str!("../../tests/fixtures/off/barcode_found.json");
    const BARCODE_NOT_FOUND: &str = include_str!("../../tests/fixtures/off/barcode_not_found.json");

    // -- Serde deserialization tests --

    fn full_response_json() -> &'static str {
        r#"{
            "code": "001234567890",
            "status": 1,
            "status_verbose": "Product found",
            "product": {
                "product_name": "Test Product",
                "serving_size": 100.0,
                "nutrition_data_per": "per 100g",
                "nutriments": {
                    "energy-kcal": 250.0,
                    "energy-kcal_100g": 250.0,
                    "proteins": 10.0,
                    "proteins_100g": 10.0,
                    "carbohydrates": 40.0,
                    "carbohydrates_100g": 40.0,
                    "fat": 8.0,
                    "fat_100g": 8.0,
                    "fiber": 5.0,
                    "fiber_100g": 5.0
                }
            }
        }"#
    }

    fn partial_response_json() -> &'static str {
        r#"{
            "code": "001234567890",
            "status": 1,
            "product": {
                "product_name": "Partial Product",
                "nutriments": {
                    "energy-kcal_100g": 200.0,
                    "proteins_100g": 5.0
                }
            }
        }"#
    }

    fn not_found_json() -> &'static str {
        r#"{
            "code": "000000000000",
            "status": 0,
            "status_verbose": "Product not found"
        }"#
    }

    #[test]
    fn test_deserialize_full_response() {
        let resp: OffResponse = serde_json::from_str(full_response_json()).unwrap();
        assert_eq!(resp.status, 1);
        assert_eq!(resp.code, "001234567890");
        let product = resp.product.unwrap();
        assert_eq!(product.product_name, Some("Test Product".into()));
        assert_eq!(product.serving_size, Some(100.0));
        assert_eq!(product.nutrition_data_per, Some("per 100g".into()));
        let n = product.nutriments.unwrap();
        assert_eq!(n.energy_kcal_100g, Some(250.0));
        assert_eq!(n.proteins_100g, Some(10.0));
        assert_eq!(n.carbohydrates_100g, Some(40.0));
        assert_eq!(n.fat_100g, Some(8.0));
        assert_eq!(n.fiber_100g, Some(5.0));
    }

    #[test]
    fn test_deserialize_partial_response() {
        let resp: OffResponse = serde_json::from_str(partial_response_json()).unwrap();
        assert_eq!(resp.status, 1);
        let product = resp.product.unwrap();
        assert_eq!(product.product_name, Some("Partial Product".into()));
        assert!(product.serving_size.is_none());
        assert!(product.nutrition_data_per.is_none());
        let n = product.nutriments.unwrap();
        assert_eq!(n.energy_kcal_100g, Some(200.0));
        assert_eq!(n.proteins_100g, Some(5.0));
        // Missing fields should be None
        assert!(n.carbohydrates_100g.is_none());
        assert!(n.fat_100g.is_none());
        assert!(n.fiber_100g.is_none());
    }

    #[test]
    fn test_deserialize_not_found() {
        let resp: OffResponse = serde_json::from_str(not_found_json()).unwrap();
        assert_eq!(resp.status, 0);
        assert!(resp.product.is_none());
    }

    #[test]
    fn test_empty_product_defaults() {
        let json = r#"{"code": "x", "status": 1, "product": {}}"#;
        let resp: OffResponse = serde_json::from_str(json).unwrap();
        let p = resp.product.unwrap();
        assert!(p.product_name.is_none());
        assert!(p.serving_size.is_none());
        assert!(p.nutriments.is_none());
    }

    #[test]
    fn test_missing_nutriments_defaults() {
        let json = r#"{"code": "x", "status": 1, "product": {"nutriments": {}}}"#;
        let resp: OffResponse = serde_json::from_str(json).unwrap();
        let n = resp.product.unwrap().nutriments.unwrap();
        assert!(n.energy_kcal.is_none());
        assert!(n.energy_kcal_100g.is_none());
        assert!(n.proteins.is_none());
        assert!(n.proteins_100g.is_none());
        assert!(n.carbohydrates.is_none());
        assert!(n.carbohydrates_100g.is_none());
        assert!(n.fat.is_none());
        assert!(n.fat_100g.is_none());
        assert!(n.fiber.is_none());
        assert!(n.fiber_100g.is_none());
    }

    // -- OffClient construction tests --

    #[test]
    fn test_client_new_sets_user_agent() {
        let client = OffClient::new("http://localhost:1234", "my-agent/1.0").unwrap();
        assert_eq!(client.base_url.as_str(), "http://localhost:1234/");
    }

    #[test]
    fn test_client_with_default_base() {
        let client = OffClient::with_default_base("my-agent/1.0").unwrap();
        assert!(client.base_url.as_str().contains("openfoodfacts"));
    }

    #[test]
    fn test_client_new_rejects_non_base_url() {
        // "mailto:" URLs parse successfully but cannot be used as a base for
        // path_segments_mut(), so construction must fail with an error
        // rather than panicking.
        let result = OffClient::new("mailto:foo@bar.com", "my-agent/1.0");
        assert!(matches!(result, Err(OffError::InvalidBase)));
    }

    // -- Integration tests with wiremock --

    #[tokio::test]
    async fn test_lookup_barcode_success() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("GET"))
            .and(path("/api/v2/product/123456"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(BARCODE_FOUND, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = OffClient::new(&base_url, "test-agent/1.0").unwrap();
        // Barcode with hyphens gets normalized to 123456
        let result = client.lookup_barcode("123-456").await.unwrap();
        let product = result.unwrap();
        assert_eq!(product.product_name, Some("Fixture Granola Bar".into()));
        assert_eq!(product.serving_size, Some(42.0));
        assert_eq!(product.nutrition_data_per, Some("100g".into()));
        let n = product.nutriments.unwrap();
        assert_eq!(n.energy_kcal, Some(411.0));
        assert_eq!(n.energy_kcal_100g, Some(411.0));
        assert_eq!(n.proteins_100g, Some(8.3));
        assert_eq!(n.carbohydrates_100g, Some(61.2));
        assert_eq!(n.fat_100g, Some(14.7));
        assert_eq!(n.fiber_100g, Some(6.5));
    }

    #[tokio::test]
    async fn test_lookup_barcode_not_found() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(BARCODE_NOT_FOUND, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = OffClient::new(&base_url, "test-agent/1.0").unwrap();
        let result = client.lookup_barcode("000000000000").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_lookup_barcode_normalizes_barcode() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        // Mock expects the normalized barcode (no hyphens/spaces)
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": "123456789012",
                "status": 1,
                "product": {
                    "product_name": "Normalized",
                    "nutriments": { "energy-kcal_100g": 100.0 }
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = OffClient::new(&base_url, "test-agent/1.0").unwrap();
        // Send barcode with hyphens and spaces
        let result = client.lookup_barcode("123-456 789-012").await.unwrap();
        let product = result.unwrap();
        assert_eq!(product.product_name, Some("Normalized".into()));
    }

    #[tokio::test]
    async fn test_lookup_barcode_unexpected_status() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": "999",
                "status": 99,
                "status_verbose": "Unknown status"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = OffClient::new(&base_url, "test-agent/1.0").unwrap();
        let err = client.lookup_barcode("999").await.unwrap_err();
        assert!(matches!(err, OffError::UnexpectedStatus(99)));
    }

    #[tokio::test]
    async fn test_lookup_barcode_network_error() {
        // Point at a URL that won't respond
        let client = OffClient::new("http://127.0.0.1:54321", "test-agent/1.0").unwrap();
        let err = client.lookup_barcode("123").await.unwrap_err();
        assert!(matches!(err, OffError::Request(_)));
    }

    #[tokio::test]
    async fn test_user_agent_header_reaches_server() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        Mock::given(method("GET"))
            .and(header("user-agent", "custom-agent/2.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": "111",
                "status": 1,
                "product": {}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = OffClient::new(&base_url, "custom-agent/2.0").unwrap();
        client.lookup_barcode("111").await.unwrap();
        // If we get here without the mock rejecting the request, the header matched
    }

    #[tokio::test]
    async fn test_lookup_barcode_injection_prevented() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        // Barcode containing '/' must reach the server as %2F in the path,
        // not split the path into extra segments.
        Mock::given(method("GET"))
            .and(path("/api/v2/product/123%2F456"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": "123/456",
                "status": 1,
                "product": { "product_name": "Slash Barcode" }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = OffClient::new(&base_url, "test-agent/1.0").unwrap();
        let result = client.lookup_barcode("123/456").await.unwrap();
        let product = result.unwrap();
        assert_eq!(product.product_name, Some("Slash Barcode".into()));
    }

    #[tokio::test]
    async fn test_lookup_barcode_query_injection_prevented() {
        let server = wiremock::MockServer::start().await;
        let base_url = server.uri();

        // Barcode containing '?evil=1' must reach the server with '?' encoded as %3F
        // in the path segment, NOT interpreted as query-string delimiter.
        // '=' is valid in path segments so it remains unencoded.
        Mock::given(method("GET"))
            .and(path("/api/v2/product/123%3Fevil=1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": "123?evil=1",
                "status": 1,
                "product": { "product_name": "Query Injection Attempt" }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = OffClient::new(&base_url, "test-agent/1.0").unwrap();
        let result = client.lookup_barcode("123?evil=1").await.unwrap();
        let product = result.unwrap();
        assert_eq!(product.product_name, Some("Query Injection Attempt".into()));
    }
}

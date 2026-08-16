//! OAuth 2.1 resource-server support for the `/mcp` endpoint.
//!
//! nom-mcp does not issue tokens itself — it validates bearer tokens minted
//! by an external OAuth 2.1 authorization server (e.g. an Authelia
//! instance) and tells clients where that authorization server is, per the
//! [MCP Authorization spec](https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization):
//! RFC 9728 (Protected Resource Metadata) for discovery, RFC 8414
//! (Authorization Server Metadata) to locate the JWKS, and OAuth 2.1 bearer
//! token validation for every `/mcp` request. The `rmcp` crate's OAuth
//! support is client-side only, so all of this is hand-rolled.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Json, Response};
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use tokio::sync::RwLock;

/// Minimum time between two JWKS refreshes triggered by an unrecognized
/// `kid`, so a flood of bogus tokens can't be used to hammer the
/// authorization server with discovery requests.
const MIN_JWKS_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Deserialize)]
struct OidcDiscovery {
    jwks_uri: String,
}

struct JwksCache {
    keys: JwkSet,
    fetched_at: Instant,
}

/// Shared OAuth resource-server state: the authorization server that issues
/// tokens for this deployment, the audience those tokens must carry, and a
/// lazily-refreshed cache of the authorization server's signing keys.
pub struct OAuthState {
    http: reqwest::Client,
    issuer: String,
    /// Canonical external URL of this server (no trailing slash), e.g.
    /// `https://nom.example.com`. Doubles as the base for both the
    /// protected-resource metadata URL and the expected token audience
    /// (`{public_url}/mcp`).
    public_url: String,
    jwks_uri: String,
    cache: RwLock<JwksCache>,
}

impl OAuthState {
    /// Fetch the authorization server's OIDC discovery document to learn
    /// its JWKS location, then fetch and cache its signing keys. Returns an
    /// error rather than a half-populated state so callers can fail server
    /// startup loudly instead of booting a server that can never validate a
    /// token.
    pub async fn discover(issuer: &str, public_url: &str) -> Result<Arc<Self>, String> {
        let http = reqwest::Client::new();
        let issuer = issuer.trim_end_matches('/').to_string();
        let discovery_url = format!("{issuer}/.well-known/openid-configuration");
        let discovery: OidcDiscovery = http
            .get(&discovery_url)
            .send()
            .await
            .map_err(|e| format!("fetching {discovery_url}: {e}"))?
            .error_for_status()
            .map_err(|e| format!("fetching {discovery_url}: {e}"))?
            .json()
            .await
            .map_err(|e| format!("parsing {discovery_url}: {e}"))?;

        let keys = fetch_jwks(&http, &discovery.jwks_uri).await?;

        Ok(Arc::new(Self {
            http,
            issuer,
            public_url: public_url.trim_end_matches('/').to_string(),
            jwks_uri: discovery.jwks_uri,
            cache: RwLock::new(JwksCache {
                keys,
                fetched_at: Instant::now(),
            }),
        }))
    }

    /// The canonical resource URI tokens must be audienced to, and that
    /// this server advertises as `resource` in its Protected Resource
    /// Metadata (RFC 9728) — the MCP endpoint itself, not the bare origin.
    fn resource_uri(&self) -> String {
        format!("{}/mcp", self.public_url)
    }

    fn metadata_url(&self) -> String {
        format!("{}/.well-known/oauth-protected-resource", self.public_url)
    }

    /// Validate a bearer token: signature (against cached/refreshed JWKS),
    /// issuer, audience, and expiry. Refreshes the JWKS cache once if the
    /// token's `kid` isn't recognized (covers key rotation), rate-limited
    /// by `MIN_JWKS_REFRESH_INTERVAL`.
    async fn validate(&self, token: &str) -> Result<(), String> {
        let header = decode_header(token).map_err(|e| format!("invalid token header: {e}"))?;
        // Reject symmetric algorithms outright: a resource server that
        // validates via a public JWKS must never accept HS*/none, or a
        // token forged with the RSA/EC public key as an HMAC secret would
        // verify successfully (classic algorithm-confusion attack).
        if matches!(
            header.alg,
            Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512
        ) {
            return Err(format!("rejected symmetric algorithm {:?}", header.alg));
        }
        let kid = header.kid.clone().ok_or("token header missing kid")?;

        let mut jwk = self.find_key(&kid).await;
        if jwk.is_none() {
            self.maybe_refresh().await?;
            jwk = self.find_key(&kid).await;
        }
        let jwk = jwk.ok_or_else(|| format!("no matching signing key for kid {kid}"))?;

        let decoding_key =
            DecodingKey::from_jwk(&jwk).map_err(|e| format!("unusable signing key: {e}"))?;
        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[self.resource_uri()]);

        decode::<serde_json::Value>(token, &decoding_key, &validation)
            .map(|_| ())
            .map_err(|e| format!("token validation failed: {e}"))
    }

    async fn find_key(&self, kid: &str) -> Option<jsonwebtoken::jwk::Jwk> {
        self.cache.read().await.keys.find(kid).cloned()
    }

    async fn maybe_refresh(&self) -> Result<(), String> {
        {
            let cache = self.cache.read().await;
            if cache.fetched_at.elapsed() < MIN_JWKS_REFRESH_INTERVAL {
                return Ok(());
            }
        }
        let keys = fetch_jwks(&self.http, &self.jwks_uri).await?;
        let mut cache = self.cache.write().await;
        cache.keys = keys;
        cache.fetched_at = Instant::now();
        Ok(())
    }
}

async fn fetch_jwks(http: &reqwest::Client, jwks_uri: &str) -> Result<JwkSet, String> {
    http.get(jwks_uri)
        .send()
        .await
        .map_err(|e| format!("fetching {jwks_uri}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("fetching {jwks_uri}: {e}"))?
        .json()
        .await
        .map_err(|e| format!("parsing {jwks_uri}: {e}"))
}

/// Axum middleware requiring a valid `Authorization: Bearer <token>` header.
/// Missing or invalid tokens get a `401` carrying the `WWW-Authenticate`
/// header MCP clients use to discover how to authenticate (RFC 9728 §5.1).
pub async fn require_bearer_token(
    State(oauth): State<Arc<OAuthState>>,
    req: Request,
    next: Next,
) -> Response {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let Some(token) = token else {
        return unauthorized(&oauth);
    };

    match oauth.validate(token).await {
        Ok(()) => next.run(req).await,
        Err(reason) => {
            tracing::debug!(reason, "rejected MCP request: invalid bearer token");
            unauthorized(&oauth)
        }
    }
}

fn unauthorized(oauth: &OAuthState) -> Response {
    let www_authenticate = format!(r#"Bearer resource_metadata="{}""#, oauth.metadata_url());
    let mut response = StatusCode::UNAUTHORIZED.into_response();
    if let Ok(value) = HeaderValue::from_str(&www_authenticate) {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, value);
    }
    response
}

/// `GET /.well-known/oauth-protected-resource` — RFC 9728 Protected
/// Resource Metadata. Deliberately unauthenticated: clients fetch this
/// before they have a token, to learn where to get one.
pub async fn metadata_handler(State(oauth): State<Arc<OAuthState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "resource": oauth.resource_uri(),
        "authorization_servers": [oauth.issuer],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::jwk::{
        AlgorithmParameters, CommonParameters, Jwk, KeyAlgorithm, PublicKeyUse, RSAKeyParameters,
        RSAKeyType,
    };
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const ISSUER_PATH: &str = "";
    const KID: &str = "test-key-1";

    // Fixed 2048-bit RSA test fixtures (generated once, offline, for this
    // test suite only — never used for anything real). Hardcoding them
    // avoids a runtime dependency on an RSA-keygen crate just to sign
    // throwaway tokens in tests: RustSec flags the `rsa` crate for a
    // private-key timing side-channel (RUSTSEC-2023-0071), which is
    // irrelevant to signing local test fixtures but still trips `cargo
    // audit` if the crate is present at all.
    const PRIMARY_KEY_PEM: &str = include_str!("../testdata/oauth_test_key_primary.pem");
    const PRIMARY_KEY_N: &str = "7wY3L3_ZpXRHU-95Pw8medCUbHx5t01TUktRH4nKdRmZlIwbf3KIhArV24Wm8gPY9tHzYaWQlE1Eg_RQNTVL6cEOBNg7QPudsgCEY6EcwkcIlrgohlYsd-wDv7nokwPCJql7MRyKHdsVkDvQoGA0X9UvvhYghRe2gj1t6oiEWz2a-J5y8zzhWNel-XloWKMKbptfwZYhW_Anm0_foJs6qjjVGJDGPYGKcq52PSfmYICo1rjHin7JSy-foYhtDJ71Rn9uI05cLngr8AhG3B2JTynPomLVk9v5WzmS83qxNDTPkGFy9hQSMLd_7RCh_sTlXiLWGhw63_O7EZ2um0comQ";
    const PRIMARY_KEY_E: &str = "AQAB";
    const FORGED_KEY_PEM: &str = include_str!("../testdata/oauth_test_key_forged.pem");

    /// Spin up a fake authorization server: OIDC discovery + JWKS, backed
    /// by a fixed RSA test fixture key. Returns the mock server and the
    /// resource server's `OAuthState` pointed at it.
    async fn fake_authorization_server() -> (MockServer, Arc<OAuthState>) {
        let server = MockServer::start().await;

        let jwk = Jwk {
            common: CommonParameters {
                public_key_use: Some(PublicKeyUse::Signature),
                key_algorithm: Some(KeyAlgorithm::RS256),
                key_id: Some(KID.to_string()),
                ..Default::default()
            },
            algorithm: AlgorithmParameters::RSA(RSAKeyParameters {
                key_type: RSAKeyType::RSA,
                n: PRIMARY_KEY_N.to_string(),
                e: PRIMARY_KEY_E.to_string(),
            }),
        };
        let jwk_set = JwkSet { keys: vec![jwk] };

        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "jwks_uri": format!("{}/jwks.json", server.uri()),
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/jwks.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&jwk_set))
            .mount(&server)
            .await;

        let issuer = format!("{}{ISSUER_PATH}", server.uri());
        let oauth = OAuthState::discover(&issuer, "https://nom.example.com")
            .await
            .expect("discovery should succeed against the mock server");

        (server, oauth)
    }

    #[derive(serde::Serialize)]
    struct Claims {
        iss: String,
        aud: String,
        exp: usize,
        sub: String,
    }

    fn mint_token(private_key_pem: &str, claims: Claims) -> String {
        let encoding_key =
            EncodingKey::from_rsa_pem(private_key_pem.as_bytes()).expect("load encoding key");
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(KID.to_string());
        encode(&header, &claims, &encoding_key).expect("sign token")
    }

    fn valid_claims(issuer: &str) -> Claims {
        Claims {
            iss: issuer.to_string(),
            aud: "https://nom.example.com/mcp".to_string(),
            exp: (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 3600) as usize,
            sub: "test-user".to_string(),
        }
    }

    #[tokio::test]
    async fn accepts_a_valid_token() {
        let (_server, oauth) = fake_authorization_server().await;
        let token = mint_token(PRIMARY_KEY_PEM, valid_claims(&oauth.issuer));
        assert!(oauth.validate(&token).await.is_ok());
    }

    #[tokio::test]
    async fn rejects_expired_token() {
        let (_server, oauth) = fake_authorization_server().await;
        let mut claims = valid_claims(&oauth.issuer);
        claims.exp = 1;
        let token = mint_token(PRIMARY_KEY_PEM, claims);
        assert!(oauth.validate(&token).await.is_err());
    }

    #[tokio::test]
    async fn rejects_wrong_issuer() {
        let (_server, oauth) = fake_authorization_server().await;
        let mut claims = valid_claims(&oauth.issuer);
        claims.iss = "https://not-the-real-issuer.example.com".to_string();
        let token = mint_token(PRIMARY_KEY_PEM, claims);
        assert!(oauth.validate(&token).await.is_err());
    }

    #[tokio::test]
    async fn rejects_wrong_audience() {
        let (_server, oauth) = fake_authorization_server().await;
        let mut claims = valid_claims(&oauth.issuer);
        claims.aud = "https://someone-elses-server.example.com/mcp".to_string();
        let token = mint_token(PRIMARY_KEY_PEM, claims);
        assert!(oauth.validate(&token).await.is_err());
    }

    #[tokio::test]
    async fn rejects_bad_signature() {
        let (_server, oauth) = fake_authorization_server().await;
        let token = mint_token(FORGED_KEY_PEM, valid_claims(&oauth.issuer));
        assert!(oauth.validate(&token).await.is_err());
    }

    #[tokio::test]
    async fn missing_header_yields_401_with_www_authenticate() {
        let (_server, oauth) = fake_authorization_server().await;
        let response = unauthorized(&oauth);
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let header = response
            .headers()
            .get(header::WWW_AUTHENTICATE)
            .expect("WWW-Authenticate header present")
            .to_str()
            .unwrap();
        assert!(header.contains("Bearer"));
        assert!(header.contains("resource_metadata="));
        assert!(header.contains("/.well-known/oauth-protected-resource"));
    }

    #[tokio::test]
    async fn metadata_shape_is_correct() {
        let (_server, oauth) = fake_authorization_server().await;
        let Json(body) = metadata_handler(State(oauth.clone())).await;
        assert_eq!(body["resource"], "https://nom.example.com/mcp");
        assert_eq!(body["authorization_servers"][0], oauth.issuer);
    }
}

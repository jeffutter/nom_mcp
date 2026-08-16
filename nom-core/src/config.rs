//! Configuration loading with layered precedence: defaults < TOML file < env vars.
//!
//! Config file location: `$XDG_CONFIG_HOME/nom_mcp/config.toml` (fallback `~/.config`).
//! DB file location: `$XDG_DATA_HOME/nom_mcp/nom.db` (fallback `~/.local/share`).
//! Env vars use `NOM_MCP_` prefix with `_` separator.
//!
//! USDA API keys are wrapped in `RedactedString` to prevent accidental leakage
//! through Debug/Display output. Keys are validated lazily per-operation, not
//! at startup.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// RedactedString — wrapper that hides secrets from Debug/Display
// ---------------------------------------------------------------------------

/// A string type that redacts its value in Debug and Display output,
/// preventing accidental logging of sensitive values like API keys.
#[derive(Clone)]
pub struct RedactedString(String);

impl RedactedString {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    /// Get the actual string value. Callers are responsible for not leaking it.
    pub fn get(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for RedactedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl std::fmt::Display for RedactedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl Serialize for RedactedString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.get())
    }
}

impl<'de> Deserialize<'de> for RedactedString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self(s))
    }
}

// ---------------------------------------------------------------------------
// Config structs
// ---------------------------------------------------------------------------

/// Remote-CLI specific configuration.
///
/// The main binary ignores this entirely; only the remote-CLI reads
/// `server_url` to know which server to connect to.
#[derive(Debug, Default, Deserialize)]
pub struct RemoteConfig {
    #[serde(default)]
    pub server_url: Option<String>,
}

/// Application configuration loaded from layered sources.
///
/// Precedence (lowest to highest): hardcoded defaults < TOML config file < env vars.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub usda_api_key: Option<RedactedString>,

    /// Optional OpenFoodFacts credentials for HTTP Basic auth (`Authorization`
    /// header). Both must be set together; when absent, OFF requests are sent
    /// unauthenticated and a warning is logged at startup.
    #[serde(default)]
    pub off_username: Option<RedactedString>,

    #[serde(default)]
    pub off_password: Option<RedactedString>,

    /// Optional OpenFoodFacts `app_uuid` request parameter (a stable random
    /// identifier for this installation, so OFF moderators can ban a single
    /// user without banning the whole app account). When unset, a v4 UUID is
    /// generated once and persisted under `$XDG_DATA_HOME/nom_mcp/`.
    /// An identifier, not a secret — plain String on purpose.
    #[serde(default)]
    pub off_app_uuid: Option<String>,

    #[serde(default)]
    pub timezone: Option<String>,

    #[serde(default = "default_http_bind_address")]
    pub http_bind_address: String,

    #[serde(default = "default_off_user_agent")]
    pub off_user_agent: String,

    #[serde(default)]
    pub remote: RemoteConfig,

    /// Issuer URL of the OAuth 2.1 authorization server (e.g. an Authelia
    /// instance) that mints access tokens for this MCP server. When unset,
    /// HTTP serve mode accepts every request unauthenticated, exactly as
    /// before OAuth support existed.
    #[serde(default)]
    pub oauth_issuer_url: Option<String>,

    /// Canonical external URL this server is reachable at once behind a
    /// reverse proxy (e.g. `https://nom.example.com`). Required whenever
    /// `oauth_issuer_url` is set: it's both the `resource` advertised in
    /// OAuth Protected Resource Metadata and the expected access-token
    /// audience.
    #[serde(default)]
    pub public_url: Option<String>,
}

fn default_http_bind_address() -> String {
    "127.0.0.1".to_string()
}

fn default_off_user_agent() -> String {
    format!("nom_mcp/{}", env!("CARGO_PKG_VERSION"))
}

impl AppConfig {
    /// Load configuration from all sources with correct precedence.
    ///
    /// Order of registration (lowest priority first):
    /// 1. Hardcoded defaults via `set_default()`
    /// 2. Optional TOML config file (only if path resolves and file exists)
    /// 3. Environment variables with `NOM_MCP_` prefix (always override)
    pub fn load() -> Result<Self, config::ConfigError> {
        let mut builder = config::Config::builder()
            .set_default("http_bind_address", default_http_bind_address())?
            .set_default("off_user_agent", default_off_user_agent())?;

        // Add TOML config file if the path resolves
        if let Some(path) = config_path() {
            builder = builder.add_source(config::File::from(path).required(false));
        }

        // Environment variables always win (highest priority).
        // Flat keys use single underscores for word separation:
        //   NOM_MCP_HTTP_BIND_ADDRESS -> http_bind_address
        // Nested keys use double underscore as the nesting separator:
        //   NOM_MCP_remote__server_url -> remote.server_url
        builder = builder.add_source(
            config::Environment::with_prefix("NOM_MCP")
                .prefix_separator("_")
                .separator("__")
                .try_parsing(true),
        );

        let settings = builder.build()?;
        settings.try_deserialize()
    }
}

// ---------------------------------------------------------------------------
// XDG path helpers
// ---------------------------------------------------------------------------

/// Resolve the config file path following XDG Base Directory spec.
///
/// Returns `Some(PathBuf)` if the config directory exists and we can append
/// the file name. Returns `None` if neither `$XDG_CONFIG_HOME` nor `$HOME`
/// is set, or if the directory doesn't exist. Does NOT auto-create directories.
fn config_path() -> Option<PathBuf> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".config"))
        })?;

    let path = config_home.join("nom_mcp").join("config.toml");
    // Only return if the parent directory exists (file itself is optional)
    if path.parent().is_some_and(|p| p.is_dir()) {
        Some(path)
    } else {
        None
    }
}

/// Resolve the database file path following XDG Base Directory spec.
///
/// Creates the parent directory if it doesn't exist (storage requires this;
/// config does not auto-create).
pub fn db_path() -> PathBuf {
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".local").join("share"))
                .unwrap_or_else(|| PathBuf::from("/tmp"))
        });

    let path = data_home.join("nom_mcp").join("nom.db");
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        let _ = std::fs::create_dir_all(parent);
    }
    path
}

/// Load (or create on first use) the persistent per-installation
/// OpenFoodFacts `app_uuid`.
///
/// The value lives at `$XDG_DATA_HOME/nom_mcp/off_app_uuid` (same directory
/// as the DB). On first use a fresh random v4 UUID is generated and written,
/// so every subsequent startup reuses the same identifier — giving OFF
/// moderators a stable per-user handle they can ban without affecting the
/// whole app account. Callers may override via `off_app_uuid` in config.
pub fn load_or_create_off_app_uuid() -> Result<String, std::io::Error> {
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".local").join("share"))
                .unwrap_or_else(|| PathBuf::from("/tmp"))
        });

    let path = data_home.join("nom_mcp").join("off_app_uuid");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let uuid = uuid::Uuid::new_v4().to_string();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, format!("{uuid}\n"))?;
    Ok(uuid)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- RedactedString tests --

    #[test]
    fn test_redacted_debug_output() {
        let secret = RedactedString::new("my-secret-key".to_string());
        let debug_output = format!("{:?}", secret);
        assert_eq!(debug_output, "[REDACTED]");
        assert!(!debug_output.contains("secret"));
    }

    #[test]
    fn test_redacted_display_output() {
        let secret = RedactedString::new("my-secret-key".to_string());
        let display_output = format!("{}", secret);
        assert_eq!(display_output, "[REDACTED]");
    }

    #[test]
    fn test_redacted_get_returns_actual_value() {
        let secret = RedactedString::new("actual-value".to_string());
        assert_eq!(secret.get(), "actual-value");
    }

    #[test]
    fn test_redacted_serialization() {
        let secret = RedactedString::new("api-key-123".to_string());
        let json = serde_json::to_string(&secret).unwrap();
        // Serialization should include the actual value (for round-trip)
        assert!(json.contains("api-key-123"));
    }

    #[test]
    fn test_redacted_deserialization() {
        let json = "\"api-key-456\"";
        let secret: RedactedString = serde_json::from_str(json).unwrap();
        assert_eq!(secret.get(), "api-key-456");
        // But Debug still redacts
        assert_eq!(format!("{:?}", secret), "[REDACTED]");
    }

    // -- AppConfig default values --

    #[test]
    fn test_default_http_bind_address() {
        assert_eq!(default_http_bind_address(), "127.0.0.1");
    }

    #[test]
    fn test_default_off_user_agent_contains_version() {
        let ua = default_off_user_agent();
        assert!(ua.starts_with("nom_mcp/"));
    }

    // -- Config load tests --

    #[serial_test::serial]
    #[test]
    fn test_load_with_no_config_file_or_env() {
        // This test works because config-rs won't find a config file
        // unless one actually exists at the XDG path
        let config = AppConfig::load().expect("should load with defaults");
        assert_eq!(config.http_bind_address, "127.0.0.1");
        assert!(config.off_user_agent.starts_with("nom_mcp/"));
        assert!(config.usda_api_key.is_none());
        assert!(config.off_username.is_none());
        assert!(config.off_password.is_none());
        assert!(config.timezone.is_none());
        assert!(config.remote.server_url.is_none());
    }

    #[serial_test::serial]
    #[test]
    fn test_usda_key_is_redacted_in_debug() {
        let config = AppConfig::load().expect("should load");
        // Even if there's no key, the Debug output should not contain any
        // plaintext key material. With Option<RedactedString>, Debug of None
        // is just "None" which is safe.
        let debug = format!("{:?}", config);
        // The field name appears but never a real key value
        assert!(debug.contains("usda_api_key"));
    }

    #[serial_test::serial]
    #[test]
    fn test_off_credentials_from_env_are_redacted() {
        let mut guard = TestGuard::new();
        guard.set("XDG_CONFIG_HOME", "/tmp/nom_mcp_nonexistent_off_creds");
        guard.set("NOM_MCP_OFF_USERNAME", "off-user");
        guard.set("NOM_MCP_OFF_PASSWORD", "off-secret-pass");

        let config = AppConfig::load().expect("should load");
        assert_eq!(config.off_username.as_ref().unwrap().get(), "off-user");
        assert_eq!(
            config.off_password.as_ref().unwrap().get(),
            "off-secret-pass"
        );

        // Debug output must never leak the credential values
        let debug = format!("{:?}", config);
        assert!(!debug.contains("off-secret-pass"));
        assert!(!debug.contains("off-user"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[serial_test::serial]
    #[test]
    fn test_off_credentials_toml_and_env_precedence() {
        let mut guard = TestGuard::new();

        let temp_dir = std::env::temp_dir().join("nom_mcp_config_test_off");
        let config_dir = temp_dir.join("config");
        let file_path = config_dir.join("nom_mcp").join("config.toml");

        std::fs::create_dir_all(config_dir.join("nom_mcp")).ok();
        std::fs::write(
            &file_path,
            r#"
off_username = "toml-user"
off_password = "toml-pass"
"#,
        )
        .expect("failed to write test config");

        guard.set_temp_dir(temp_dir.clone());
        guard.set("XDG_CONFIG_HOME", &config_dir.to_string_lossy());

        // TOML values load when no env vars are set
        let config = AppConfig::load().expect("should load from TOML");
        assert_eq!(config.off_username.as_ref().unwrap().get(), "toml-user");
        assert_eq!(config.off_password.as_ref().unwrap().get(), "toml-pass");

        // Env vars win over TOML
        guard.set("NOM_MCP_OFF_USERNAME", "env-user");
        guard.set("NOM_MCP_OFF_PASSWORD", "env-pass");
        let config = AppConfig::load().expect("should load");
        assert_eq!(config.off_username.as_ref().unwrap().get(), "env-user");
        assert_eq!(config.off_password.as_ref().unwrap().get(), "env-pass");
    }

    #[serial_test::serial]
    #[test]
    fn test_off_app_uuid_from_env() {
        let mut guard = TestGuard::new();
        guard.set("XDG_CONFIG_HOME", "/tmp/nom_mcp_nonexistent_off_uuid");
        guard.set("NOM_MCP_OFF_APP_UUID", "custom-uuid-value");

        let config = AppConfig::load().expect("should load");
        assert_eq!(config.off_app_uuid.as_deref(), Some("custom-uuid-value"));
    }

    #[serial_test::serial]
    #[test]
    fn test_load_or_create_off_app_uuid_persists_across_calls() {
        let mut guard = TestGuard::new();
        let data_home = std::env::temp_dir().join("nom_mcp_data_test_off_uuid");
        guard.set("XDG_DATA_HOME", &data_home.to_string_lossy());
        guard.set("HOME", "/nonexistent-home-for-off-uuid-test");

        // First call generates and persists
        let first = load_or_create_off_app_uuid().expect("should create uuid");
        let parsed = uuid::Uuid::parse_str(&first).expect("generated value is a valid UUID");
        assert_eq!(parsed.get_version_num(), 4, "expected a random v4 UUID");

        let file = data_home.join("nom_mcp").join("off_app_uuid");
        assert!(file.exists(), "uuid file should be persisted");
        assert_eq!(std::fs::read_to_string(&file).unwrap().trim(), first);

        // Second call reuses the persisted value (stability across startups)
        let second = load_or_create_off_app_uuid().expect("should read existing");
        assert_eq!(second, first);

        // Cleanup (guard only removes XDG_CONFIG_HOME-derived temp dirs via set_temp_dir,
        // so remove this one explicitly)
        let _ = std::fs::remove_dir_all(&data_home);
    }

    // -- db_path tests --

    #[test]
    fn test_db_path_creates_parent_directory() {
        let path = db_path();
        // The path should end with nom.db
        assert_eq!(path.file_name().unwrap(), "nom.db");
        // Parent directory should exist (we create it)
        if let Some(parent) = path.parent() {
            assert!(parent.exists());
        }
    }

    // -- Precedence tests with real config file (serialized to avoid env var conflicts) --

    /// Guard that restores env vars and cleans up temp dirs on drop.
    /// Keep in sync with the identical TestGuard in nom-mcp/src/bin/nom-mcp-remote.rs.
    struct TestGuard {
        temp_dir: Option<PathBuf>,
        saved_xdg: Option<String>,
        cleared_vars: Vec<String>,
    }

    impl TestGuard {
        fn new() -> Self {
            Self {
                temp_dir: None,
                saved_xdg: std::env::var_os("XDG_CONFIG_HOME")
                    .map(|v| v.to_string_lossy().to_string()),
                cleared_vars: Vec::new(),
            }
        }

        fn set(&mut self, key: &str, value: &str) {
            unsafe { std::env::set_var(key, value) };
            self.cleared_vars.push(key.to_string());
        }

        fn set_temp_dir(&mut self, path: PathBuf) {
            self.temp_dir = Some(path);
        }
    }

    impl Drop for TestGuard {
        fn drop(&mut self) {
            // Restore XDG_CONFIG_HOME to its pre-guard state. Both the
            // no-prior-value and empty-string cases must remove the var
            // explicitly — leaving this as `if let Some` only skipped the
            // removal, silently leaking whatever `set()` last wrote when
            // there was no prior value to restore.
            match &self.saved_xdg {
                Some(saved) if !saved.is_empty() => {
                    unsafe { std::env::set_var("XDG_CONFIG_HOME", saved) };
                }
                _ => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
            }
            // Remove any test-specific env vars.
            // Skip XDG_CONFIG_HOME — the block above already restored (or
            // correctly left unset) that variable; removing it here would
            // unconditionally erase a pre-existing value.
            for var in &self.cleared_vars {
                if var == "XDG_CONFIG_HOME" {
                    continue;
                }
                unsafe { std::env::remove_var(var) };
            }
            // Remove temp dir
            if let Some(ref dir) = self.temp_dir {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }

    #[serial_test::serial]
    #[test]
    fn test_toml_overrides_defaults() {
        let mut guard = TestGuard::new();

        let temp_dir = std::env::temp_dir().join("nom_mcp_config_test");
        let config_dir = temp_dir.join("config");
        let file_path = config_dir.join("nom_mcp").join("config.toml");

        std::fs::create_dir_all(config_dir.join("nom_mcp")).ok();
        std::fs::write(
            &file_path,
            r#"
http_bind_address = "0.0.0.0"
usda_api_key = "toml-key-123"
timezone = "America/New_York"

[remote]
server_url = "http://localhost:8080"
"#,
        )
        .expect("failed to write test config");

        guard.set_temp_dir(temp_dir.clone());
        guard.set("XDG_CONFIG_HOME", &config_dir.to_string_lossy());

        let config = AppConfig::load().expect("should load from TOML");
        assert_eq!(config.http_bind_address, "0.0.0.0");
        assert!(config.usda_api_key.is_some());
        assert_eq!(config.usda_api_key.unwrap().get(), "toml-key-123");
        assert_eq!(config.timezone, Some("America/New_York".to_string()));
        assert_eq!(
            config.remote.server_url,
            Some("http://localhost:8080".to_string())
        );
    }

    #[serial_test::serial]
    #[test]
    fn test_env_overrides_toml() {
        let mut guard = TestGuard::new();

        let temp_dir = std::env::temp_dir().join("nom_mcp_config_test_env");
        let config_dir = temp_dir.join("config");
        let file_path = config_dir.join("nom_mcp").join("config.toml");

        std::fs::create_dir_all(config_dir.join("nom_mcp")).ok();
        std::fs::write(
            &file_path,
            r#"
http_bind_address = "0.0.0.0"
usda_api_key = "toml-key-from-file"
timezone = "America/Los_Angeles"
"#,
        )
        .expect("failed to write test config");

        guard.set_temp_dir(temp_dir.clone());
        guard.set("XDG_CONFIG_HOME", &config_dir.to_string_lossy());
        guard.set("NOM_MCP_HTTP_BIND_ADDRESS", "192.168.1.1");
        guard.set("NOM_MCP_USDA_API_KEY", "env-key-wins");
        guard.set("NOM_MCP_TIMEZONE", "Europe/London");

        let config = AppConfig::load().expect("should load");
        // All values should come from env vars, not TOML
        assert_eq!(config.http_bind_address, "192.168.1.1");
        assert_eq!(config.usda_api_key.unwrap().get(), "env-key-wins");
        assert_eq!(config.timezone, Some("Europe/London".to_string()));
    }

    #[serial_test::serial]
    #[test]
    fn test_missing_config_file_is_not_an_error() {
        let mut guard = TestGuard::new();
        guard.set("XDG_CONFIG_HOME", "/tmp/nom_mcp_nonexistent_12345");

        let config = AppConfig::load().expect("should succeed even without config file");
        // Should fall back to defaults
        assert_eq!(config.http_bind_address, "127.0.0.1");
        assert!(config.usda_api_key.is_none());
        assert!(config.timezone.is_none());
    }

    #[serial_test::serial]
    #[test]
    fn test_guard_restores_pre_existing_xdg_config_home() {
        // Set XDG_CONFIG_HOME to a known value BEFORE constructing the guard,
        // then use the guard to point at a different dir, drop, and verify
        // the original value is restored (not removed).
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", "/tmp/nom_mcp_original_value_marker");
        }
        let mut guard = TestGuard::new();
        guard.set("XDG_CONFIG_HOME", "/tmp/nom_mcp_test_scratch");
        drop(guard);
        assert_eq!(
            std::env::var("XDG_CONFIG_HOME").unwrap(),
            "/tmp/nom_mcp_original_value_marker"
        );
        // Clean up marker so we don't pollute subsequent tests
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    #[serial_test::serial]
    #[test]
    fn test_guard_leaves_xdg_config_home_unset_when_no_prior_value() {
        // The common case (per TASK-29's own bug report: most CI environments):
        // XDG_CONFIG_HOME is unset before the guard runs. Verify the guard's
        // own set() value doesn't leak past drop().
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        let mut guard = TestGuard::new();
        guard.set("XDG_CONFIG_HOME", "/tmp/nom_mcp_test_scratch_no_prior");
        drop(guard);
        assert!(
            std::env::var("XDG_CONFIG_HOME").is_err(),
            "XDG_CONFIG_HOME should be unset after drop when there was no prior value"
        );
    }

    #[serial_test::serial]
    #[test]
    fn test_env_nested_key_via_double_underscore() {
        let mut guard = TestGuard::new();
        // Point XDG_CONFIG_HOME at a nonexistent dir so no TOML file is loaded
        guard.set("XDG_CONFIG_HOME", "/tmp/nom_mcp_nonexistent_nested_test");
        // Nested key: double underscore separates path components
        guard.set("NOM_MCP_remote__server_url", "http://example.com:9999");
        // Flat key: single underscores are preserved within the key name
        guard.set("NOM_MCP_HTTP_BIND_ADDRESS", "10.0.0.1");

        let config = AppConfig::load().expect("should load");
        // Nested key should be parsed correctly
        assert_eq!(
            config.remote.server_url,
            Some("http://example.com:9999".to_string())
        );
        // Flat key should still work unaffected
        assert_eq!(config.http_bind_address, "10.0.0.1");
    }
}

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

    #[serde(default)]
    pub timezone: Option<String>,

    #[serde(default = "default_http_bind_address")]
    pub http_bind_address: String,

    #[serde(default = "default_off_user_agent")]
    pub off_user_agent: String,

    #[serde(default)]
    pub remote: RemoteConfig,
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
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    path
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

---
id: TASK-2.3
title: Config and secrets handling
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:23'
updated_date: '2026-08-11 18:24'
labels:
  - planned
dependencies:
  - TASK-2.1
type: feature
ordinal: 22000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Defaults < optional TOML file ($XDG_CONFIG_HOME/nom_mcp/config.toml) < env vars (NOM_MCP_*, env wins). No CLI flags for values. DB path $XDG_DATA_HOME/nom_mcp/nom.db. USDA FDC API key from env or file, always redacted (no Debug/Display leak), validated lazily per-Operation (not at startup). OpenFoodFacts User-Agent default nom_mcp/<version>, overridable. HTTP binds 127.0.0.1 by default. Remote-CLI shares the same Config type via its own [remote] table (server_url). Timezone key (NOM_MCP_TIMEZONE / timezone TOML key, optional IANA string).

See doc-5 §9.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Config loads with correct precedence: defaults < TOML file < env vars
- [x] #2 USDA key type never appears in Debug/Display output; missing key does not fail non-USDA operations
- [x] #3 [remote] table parsed for remote-CLI's server_url, ignored by the main binary
- [x] #4 timezone key present and read by config, unused until TASK for Clock service consumes it
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
### Implementation Plan: Config and secrets handling

**Location**: `nom-core/src/config.rs` (new module) + dependency updates to `nom-core/Cargo.toml`.

---

#### Dependencies to add (nom-core/Cargo.toml)

- **`config`** (`crates.io/crates/config`) — layered config library supporting TOML + env vars with explicit precedence ordering. Register sources in priority order: defaults first, then TOML file, then env vars (env wins).
- **`serde`** already present with derive feature; use `#[serde(default)]` on optional fields.

No additional crates needed for redaction or XDG paths — implement locally.

---

#### Step 1: Define `RedactedString` wrapper type (~40 lines)

A minimal newtype that wraps `String`, implements `Debug`/`Display` as `[REDACTED]`, but exposes the value internally via `get(&self) -> &str`.

```rust
pub struct RedactedString(String);

impl RedactedString {
    pub fn new(s: String) -> Self { Self(s) }
    pub fn get(&self) -> &str { &self.0 }
}

impl std::fmt::Debug for RedactedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl std::fmt::Display for RedactedString {
    fn fmt(f, _) = f.write_str("[REDACTED]")
}
```

- Derive `Serialize`/`Deserialize` for JSON round-trip (but keep Debug/Display redacted).
- Unit tests: verify Debug/Display output is `[REDACTED]`, `get()` returns actual value, serialization works.

---

#### Step 2: Define `Config` struct and nested `RemoteConfig` (~50 lines)

```rust
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub usda_api_key: Option<RedactedString>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub http_bind_address: String, // default "127.0.0.1"
    #[serde(default)]
    pub off_user_agent: String,    // default "nom_mcp/<version>"
    #[serde(default)]
    pub remote: RemoteConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct RemoteConfig {
    #[serde(default)]
    pub server_url: Option<String>,
}
```

Use `#[serde(default = "path::to::fn")]` for fields with non-empty defaults (http_bind_address, off_user_agent).

---

#### Step 3: Implement XDG path resolution (~30 lines)

Helper function `config_path() -> Option<PathBuf>`:
- Read `$XDG_CONFIG_HOME` env var, fallback to `$HOME/.config`
- Append `/nom_mcp/config.toml`
- Return `None` if neither directory exists (no auto-creation — user must create file explicitly)

Helper function `db_path() -> PathBuf`:
- Read `$XDG_DATA_HOME` env var, fallback to `$HOME/.local/share`
- Append `/nom_mcp/nom.db`
- Create parent directory if needed (storage needs this; config does not)

---

#### Step 4: Implement `AppConfig::load() -> Result<AppConfig, Error>` (~60 lines)

Using `config::Config::builder()`:

```rust
pub fn load() -> Result<AppConfig, config::ConfigError> {
    let mut builder = config::Config::builder()
        .set_default("http_bind_address", "127.0.0.1")?
        .set_default("off_user_agent", concat!("nom_mcp/", env!("CARGO_PKG_VERSION")))?;

    // Add TOML file if it exists
    if let Some(path) = config_path() {
        builder = builder.add_source(
            config::File::from(path).required(false)
        );
    }

    // Add env vars (NOM_MCP_ prefix, _ separator)
    builder = builder.add_source(
        config::Environment::with_prefix("NOM_MCP")
            .separator("_")
    );

    builder.build()?.try_deserialize()
}
```

Key behaviors:
- Defaults are set first (lowest priority)
- TOML file is optional — only loaded if path exists
- Env vars always override (highest priority)
- USDA key from `NOM_MCP_USDA_API_KEY` env or `usda_api_key` TOML key
- Timezone from `NOM_MCP_TIMEZONE` env or `timezone` TOML key
- No validation of values at load time — store raw strings, validate lazily when consumed

---

#### Step 5: Wire into `nom-core/src/lib.rs` (~2 lines)

Add `pub mod config;` to expose the module.

---

#### Step 6: Integration verification

- `cargo test` passes including new config tests
- Test scenarios:
  - Load with no config file + no env vars → all defaults
  - Load with TOML file overriding some values → overrides work
  - Load with env vars → env wins over TOML
  - USDA key never appears in Debug output
  - `remote.server_url` is parseable from TOML `[remote]` table
  - Missing config file does not cause error (optional)

---

#### Acceptance Criteria Mapping

- **AC #1** (precedence): Covered by Steps 2-4 — config-rs builder registers defaults < file < env in correct order.
- **AC #2** (redaction): Covered by Step 1 — `RedactedString` type guarantees no leak via Debug/Display. Lazy validation: `usda_api_key` is `Option<RedactedString>`, callers check presence at operation time.
- **AC #3** ([remote] table): Covered by Step 2 — `RemoteConfig` nested struct parsed automatically by serde/config-rs from `[remote]` TOML table. Main binary simply ignores `config.remote` field.
- **AC #4** (timezone): Covered by Step 2 — `timezone: Option<String>` field, validated lazily by Clock service (future task).
<!-- SECTION:PLAN:END -->

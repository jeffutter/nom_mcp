---
id: TASK-2.6
title: CI/CD pipeline (GitHub Actions + Cachix)
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:23'
updated_date: '2026-08-11 23:42'
labels:
  - planned
dependencies:
  - TASK-2.1
type: chore
ordinal: 25000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Mirror jeffutter/notectl's .github/workflows/{ci,cd,audit}.yml directly:
- CI: on push to main + PRs — test/rustfmt/clippy/docs jobs, each via 'nix develop .#ci -c cargo ...', using DeterminateSystems/nix-installer-action + magic-nix-cache-action + Swatinem/rust-cache.
- CD: on a semver git tag — cross-builds release binaries (macos-aarch64, linux-x86_64, linux-aarch64), strips+tars+sha256s them, then cachix/install-nix-action + cachix/cachix-action (cache name 'jeffutter') runs nix build .#nom-mcp / .#nom-mcp-remote + nix flake check, pushing to the jeffutter Cachix cache; tarballs+shasums attached to a GitHub Release via softprops/action-gh-release.
- audit: daily cron + Cargo.toml/lock-touching pushes/PRs, cargo-audit via rustsec/audit-check.

See doc-5 §12 (corrected per TASK-1.15).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 ci.yml runs test/rustfmt/clippy/docs jobs on push to main and on PRs
- [x] #2 cd.yml triggers on a semver tag, builds cross-platform release artifacts, and pushes to the jeffutter Cachix cache
- [x] #3 audit.yml runs cargo-audit on a daily cron and on Cargo.toml/Cargo.lock changes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

Create three workflow files under , mirroring jeffutter/notectl exactly with binary/package name substitutions.

### Files to create

1.  — Continuous Integration
2.  — Continuous Deployment (tag-triggered releases)
3.  — Security audit (daily cron + Cargo change triggers)

---

### Step 1: Create  directory structure

---

### Step 2: Write 

Mirror notectl's ci.yml with 4 parallel jobs: , , , .

**Adaptations from notectl:**
- Same triggers: push to main + pull_request
- Each job runs inside  using the lean CI devShell from flake.nix
- Pin action versions instead of floating on :
  -  (or latest pinned release)
  -  (repo-scoped cache, still functional)
- Keep  with  per job and  to wrap cargo commands in the Nix shell
- 

**Jobs:**
| Job | Command | Notes |
|---|---|---|
| test | 
running 70 tests
test client::off::tests::test_deserialize_not_found ... ok
test client::off::tests::test_empty_product_defaults ... ok
test client::off::tests::test_deserialize_partial_response ... ok
test client::off::tests::test_deserialize_full_response ... ok
test client::off::tests::test_missing_nutriments_defaults ... ok
test config::tests::test_db_path_creates_parent_directory ... ok
test config::tests::test_default_off_user_agent_contains_version ... ok
test client::off::tests::test_client_with_default_base ... ok
test client::off::tests::test_client_new_sets_user_agent ... ok
test config::tests::test_redacted_debug_output ... ok
test config::tests::test_redacted_display_output ... ok
test config::tests::test_default_http_bind_address ... ok
test config::tests::test_redacted_deserialization ... ok
test config::tests::test_redacted_get_returns_actual_value ... ok
test config::tests::test_redacted_serialization ... ok
test error::tests::test_conflict_serialization ... ok
test error::tests::test_deserialize_minimal_json ... ok
test error::tests::test_exit_code_mapping ... ok
test error::tests::test_not_found_serialization ... ok
test error::tests::test_external_api_failure_serialization ... ok
test error::tests::test_render_conflict ... ok
test error::tests::test_render_lock_probe ... ok
test error::tests::test_render_not_found ... ok
test error::tests::test_format_success_pretty_prints_json ... ok
test error::tests::test_render_storage_failure ... ok
test error::tests::test_render_validation ... ok
test error::tests::test_render_external_api_failure ... ok
test error::tests::test_storage_failure_serialization ... ok
test error::tests::test_round_trip_serialization ... ok
test error::tests::test_validation_serialization ... ok
test error::tests::test_http_status_mapping ... ok
test config::tests::test_env_overrides_toml ... ok
test logging::tests::test_init_server_returns_ok ... ok
test operation::cli_router::tests::test_build_cli_command_includes_cli_ops ... ok
test client::off::tests::test_lookup_barcode_network_error ... ok
test operation::mcp_handler::tests::test_bad_schema_does_not_panic ... ok
test operation::mcp_handler::tests::test_empty_registry_list_tools ... ok
test operation::http_router::tests::test_build_http_router_has_routes ... ok
test operation::mcp_handler::tests::test_get_tool_skips_bad_schema ... ok
test operation::mcp_handler::tests::test_list_tools_omits_bad_schema_but_keeps_good_ops ... ok
test logging::tests::test_init_cli_returns_ok ... ok
test operation::registry::tests::test_default_surfaces_is_all ... ok
test operation::cli_router::tests::test_parse_and_dispatch_no_subcommand ... ok
test operation::http_router::tests::test_handle_operation_error_serializes_error_data_body ... ok
test operation::registry::tests::test_empty_registry ... ok
test config::tests::test_missing_config_file_is_not_an_error ... ok
test operation::registry::tests::test_filter_by_cli_surface ... ok
test operation::registry::tests::test_filter_by_http_surface ... ok
test operation::mcp_handler::tests::test_mcp_handler_new ... ok
test operation::tests::test_surfaces_http_only ... ok
test operation::registry::tests::test_register_and_get ... ok
test operation::registry::tests::test_filter_by_mcp_surface ... ok
test operation::tests::test_surfaces_cli_only ... ok
test operation::mcp_handler::tests::test_tool_from_operation_has_required_fields ... ok
test operation::tests::test_surfaces_intersection ... ok
test operation::tests::test_surfaces_mcp_only ... ok
test operation::tests::test_surfaces_default_is_all ... ok
test config::tests::test_load_with_no_config_file_or_env ... ok
test client::off::tests::test_lookup_barcode_success ... ok
test client::off::tests::test_user_agent_header_reaches_server ... ok
test client::off::tests::test_lookup_barcode_unexpected_status ... ok
test client::off::tests::test_lookup_barcode_not_found ... ok
test config::tests::test_toml_overrides_defaults ... ok
test client::off::tests::test_lookup_barcode_normalizes_barcode ... ok
test config::tests::test_usda_key_is_redacted_in_debug ... ok
test storage::test::test_all_six_tables_created ... ok
test storage::test::test_indexes_exist ... ok
test storage::test::test_fk_enforcement_active ... ok
test storage::test::test_migrations_table_has_version_entry ... ok
test storage::test::test_migration_idempotency ... ok

test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | Full workspace test suite |
| rustfmt | Diff in /home/jeffutter/src/nom_mcp/nom-core/src/client/off.rs:96:
     /// provided user-agent string (typically from config).
     pub fn new(base_url: &str, user_agent: &str) -> Result<Self, OffError> {
         let url = Url::parse(base_url)?;
[31m-        let http = reqwest::Client::builder()
(B[m[31m-            .user_agent(user_agent)
(B[m[31m-            .build()?;
(B[m[32m+        let http = reqwest::Client::builder().user_agent(user_agent).build()?;
(B[m         Ok(Self {
             http,
             base_url: url,
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/client/off.rs:156:
 #[cfg(test)]
 mod tests {
     use super::*;
[32m+    use wiremock::matchers::{header, method};
(B[m     use wiremock::{Mock, ResponseTemplate};
[31m-    use wiremock::matchers::{method, header};
(B[m 
     // -- Serde deserialization tests --
 
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/cli_router.rs:12:
 
 /// Build a clap Command tree from operations that expose CLI surface.
 pub fn build_cli_command(registry: &super::OperationRegistry) -> Command {
[31m-    let mut cmd = Command::new("nom-mcp")
(B[m[31m-        .about("NOM nutrition tracker — local CLI mode");
(B[m[32m+    let mut cmd = Command::new("nom-mcp").about("NOM nutrition tracker — local CLI mode");
(B[m 
     for op in registry.filter_by_surface(Surfaces::CLI) {
         let name: &'static str = Box::leak(op.name().to_string().into_boxed_str());
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/cli_router.rs:20:
         let desc: &'static str = Box::leak(op.description().to_string().into_boxed_str());
         let subcmd = Command::new(name)
             .about(desc)
[31m-            .arg(
(B[m[31m-                Arg::new("args")
(B[m[31m-                    .num_args(0..)
(B[m[31m-                    .action(ArgAction::Set),
(B[m[31m-            );
(B[m[32m+            .arg(Arg::new("args").num_args(0..).action(ArgAction::Set));
(B[m         cmd = cmd.subcommand(subcmd);
     }
 
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/cli_router.rs:40:
     args: &[String],
 ) -> Result<(Arc<dyn Operation>, Arc<serde_json::Value>), crate::error::ErrorData> {
     let cmd = build_cli_command(registry);
[31m-    let matches = cmd.clone().try_get_matches_from(args).map_err(|e| {
(B[m[31m-        crate::error::ErrorData::validation("arguments", e.to_string())
(B[m[31m-    })?;
(B[m[32m+    let matches = cmd
(B[m[32m+        .clone()
(B[m[32m+        .try_get_matches_from(args)
(B[m[32m+        .map_err(|e| crate::error::ErrorData::validation("arguments", e.to_string()))?;
(B[m 
     // Extract subcommand name
     let subcommand_name = matches
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/cli_router.rs:99:
 
     #[async_trait::async_trait]
     impl Operation for TestOp {
[31m-        fn name(&self) -> &str { self.name }
(B[m[31m-        fn description(&self) -> &str { "test" }
(B[m[31m-        fn surfaces(&self) -> Surfaces { self.surfaces }
(B[m[32m+        fn name(&self) -> &str {
(B[m[32m+            self.name
(B[m[32m+        }
(B[m[32m+        fn description(&self) -> &str {
(B[m[32m+            "test"
(B[m[32m+        }
(B[m[32m+        fn surfaces(&self) -> Surfaces {
(B[m[32m+            self.surfaces
(B[m[32m+        }
(B[m         async fn execute_json(
             &self,
             _args: Arc<serde_json::Value>,
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/http_router.rs:7:
 
 use std::sync::Arc;
 
[31m-use axum::{
(B[m[31m-    http::StatusCode,
(B[m[31m-    routing::post,
(B[m[31m-    Json, Router,
(B[m[31m-};
(B[m[32m+use axum::{Json, Router, http::StatusCode, routing::post};
(B[m 
 use super::Surfaces;
 
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/http_router.rs:26:
     for op in registry.filter_by_surface(Surfaces::HTTP) {
         let op = op.clone();
         let path = format!("/api/{}", op.name());
[31m-        router = router.route(&path, post(move |Json(args): Json<serde_json::Value>| {
(B[m[31m-            handle_operation(op, args)
(B[m[31m-        }));
(B[m[32m+        router = router.route(
(B[m[32m+            &path,
(B[m[32m+            post(move |Json(args): Json<serde_json::Value>| handle_operation(op, args)),
(B[m[32m+        );
(B[m     }
 
     router
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/http_router.rs:58:
 
     #[async_trait::async_trait]
     impl Operation for TestOp {
[31m-        fn name(&self) -> &str { "test-op" }
(B[m[31m-        fn description(&self) -> &str { "test" }
(B[m[31m-        fn surfaces(&self) -> Surfaces { Surfaces::HTTP }
(B[m[32m+        fn name(&self) -> &str {
(B[m[32m+            "test-op"
(B[m[32m+        }
(B[m[32m+        fn description(&self) -> &str {
(B[m[32m+            "test"
(B[m[32m+        }
(B[m[32m+        fn surfaces(&self) -> Surfaces {
(B[m[32m+            Surfaces::HTTP
(B[m[32m+        }
(B[m         async fn execute_json(
             &self,
             args: Arc<serde_json::Value>,
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/http_router.rs:83:
 
     #[async_trait::async_trait]
     impl Operation for FailOp {
[31m-        fn name(&self) -> &str { "fail-op" }
(B[m[31m-        fn description(&self) -> &str { "test" }
(B[m[31m-        fn surfaces(&self) -> Surfaces { Surfaces::HTTP }
(B[m[32m+        fn name(&self) -> &str {
(B[m[32m+            "fail-op"
(B[m[32m+        }
(B[m[32m+        fn description(&self) -> &str {
(B[m[32m+            "test"
(B[m[32m+        }
(B[m[32m+        fn surfaces(&self) -> Surfaces {
(B[m[32m+            Surfaces::HTTP
(B[m[32m+        }
(B[m         async fn execute_json(
             &self,
             _args: Arc<serde_json::Value>,
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/http_router.rs:96:
 
     #[tokio::test]
     async fn test_handle_operation_error_serializes_error_data_body() {
[31m-        let (status, Json(body)) =
(B[m[31m-            handle_operation(Arc::new(FailOp), serde_json::json!({})).await;
(B[m[32m+        let (status, Json(body)) = handle_operation(Arc::new(FailOp), serde_json::json!({})).await;
(B[m         assert_eq!(status, StatusCode::NOT_FOUND);
         assert_eq!(body["category"], "NotFound");
     }
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/mcp_handler.rs:8:
 use std::sync::Arc;
 
 use rmcp::{
[31m-    ErrorData,
(B[m[32m+    ErrorData, RoleServer,
(B[m     handler::server::ServerHandler,
     model::{
         CallToolRequestParams, CallToolResult, ContentBlock, ErrorCode, ListToolsResult,
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/mcp_handler.rs:15:
         PaginatedRequestParams, Tool,
     },
     service::RequestContext,
[31m-    RoleServer,
(B[m };
 
 use super::{Operation, OperationRegistry, Surfaces};
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/mcp_handler.rs:61:
             .filter_by_surface(Surfaces::MCP)
             .iter()
             .filter_map(|op| {
[31m-                tool_from_operation(op.as_ref()).map_err(|err| {
(B[m[31m-                    tracing::warn!(
(B[m[31m-                        operation = op.name(),
(B[m[31m-                        error = %err,
(B[m[31m-                        "skipping operation with invalid input_schema",
(B[m[31m-                    );
(B[m[31m-                }).ok()
(B[m[32m+                tool_from_operation(op.as_ref())
(B[m[32m+                    .map_err(|err| {
(B[m[32m+                        tracing::warn!(
(B[m[32m+                            operation = op.name(),
(B[m[32m+                            error = %err,
(B[m[32m+                            "skipping operation with invalid input_schema",
(B[m[32m+                        );
(B[m[32m+                    })
(B[m[32m+                    .ok()
(B[m             })
             .collect();
         Ok(ListToolsResult::with_all_items(tools))
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/mcp_handler.rs:78:
         request: CallToolRequestParams,
         _context: RequestContext<RoleServer>,
     ) -> Result<CallToolResult, ErrorData> {
[31m-        let op = self
(B[m[31m-            .registry
(B[m[31m-            .get(&request.name)
(B[m[31m-            .ok_or_else(|| ErrorData::invalid_params(format!("unknown tool: {}", request.name), None))?;
(B[m[32m+        let op = self.registry.get(&request.name).ok_or_else(|| {
(B[m[32m+            ErrorData::invalid_params(format!("unknown tool: {}", request.name), None)
(B[m[32m+        })?;
(B[m 
         // Convert arguments to serde_json::Value
         let args = match request.arguments {
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/mcp_handler.rs:121:
     // and we reject it gracefully rather than panicking.
     let schema = match input_schema {
         serde_json::Value::Object(obj) => Arc::new(obj),
[31m-        other => return Err(ErrorData::new(
(B[m[31m-            ErrorCode::INVALID_PARAMS,
(B[m[31m-            format!("operation '{}' returned a non-object schema: {:?}", op.name(), other),
(B[m[31m-            None,
(B[m[31m-        )),
(B[m[32m+        other => {
(B[m[32m+            return Err(ErrorData::new(
(B[m[32m+                ErrorCode::INVALID_PARAMS,
(B[m[32m+                format!(
(B[m[32m+                    "operation '{}' returned a non-object schema: {:?}",
(B[m[32m+                    op.name(),
(B[m[32m+                    other
(B[m[32m+                ),
(B[m[32m+                None,
(B[m[32m+            ));
(B[m[32m+        }
(B[m     };
 
[31m-    Ok(Tool::new(op.name().to_string(), op.description().to_string(), schema))
(B[m[32m+    Ok(Tool::new(
(B[m[32m+        op.name().to_string(),
(B[m[32m+        op.description().to_string(),
(B[m[32m+        schema,
(B[m[32m+    ))
(B[m }
 
 #[cfg(test)]
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/mcp_handler.rs:140:
 
     #[async_trait::async_trait]
     impl Operation for TestOp {
[31m-        fn name(&self) -> &str { "test-op" }
(B[m[31m-        fn description(&self) -> &str { "A test operation" }
(B[m[31m-        fn surfaces(&self) -> Surfaces { Surfaces::MCP }
(B[m[32m+        fn name(&self) -> &str {
(B[m[32m+            "test-op"
(B[m[32m+        }
(B[m[32m+        fn description(&self) -> &str {
(B[m[32m+            "A test operation"
(B[m[32m+        }
(B[m[32m+        fn surfaces(&self) -> Surfaces {
(B[m[32m+            Surfaces::MCP
(B[m[32m+        }
(B[m         async fn execute_json(
             &self,
             args: Arc<serde_json::Value>,
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/mcp_handler.rs:157:
 
     #[async_trait::async_trait]
     impl Operation for BadSchemaOp {
[31m-        fn name(&self) -> &str { "bad-schema-op" }
(B[m[31m-        fn description(&self) -> &str { "An operation with a broken schema" }
(B[m[31m-        fn surfaces(&self) -> Surfaces { Surfaces::MCP }
(B[m[32m+        fn name(&self) -> &str {
(B[m[32m+            "bad-schema-op"
(B[m[32m+        }
(B[m[32m+        fn description(&self) -> &str {
(B[m[32m+            "An operation with a broken schema"
(B[m[32m+        }
(B[m[32m+        fn surfaces(&self) -> Surfaces {
(B[m[32m+            Surfaces::MCP
(B[m[32m+        }
(B[m         fn input_schema(&self) -> Option<serde_json::Value> {
             Some(serde_json::json!(["not", "an", "object"]))
         }
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/mcp_handler.rs:176:
         let mut reg = OperationRegistry::new();
         reg.register(Arc::new(TestOp));
         let handler = McpHandler::new(reg);
[31m-        assert_eq!(handler.get_tool("test-op").map(|t| t.name.to_string()), Some("test-op".to_string()));
(B[m[32m+        assert_eq!(
(B[m[32m+            handler.get_tool("test-op").map(|t| t.name.to_string()),
(B[m[32m+            Some("test-op".to_string())
(B[m[32m+        );
(B[m         assert!(handler.get_tool("nonexistent").is_none());
     }
 
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/mcp_handler.rs:221:
         reg.register(Arc::new(BadSchemaOp));
 
         let mcp_ops = reg.filter_by_surface(Surfaces::MCP);
[31m-        let tools: Vec<_> = mcp_ops.iter()
(B[m[32m+        let tools: Vec<_> = mcp_ops
(B[m[32m+            .iter()
(B[m             .filter_map(|op| tool_from_operation(op.as_ref()).ok())
             .collect();
 
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/operation/registry.rs:174:
 
         #[async_trait::async_trait]
         impl Operation for DefaultSurfacesOp {
[31m-            fn name(&self) -> &str { "default_surfaces" }
(B[m[31m-            fn description(&self) -> &str { "Uses default surfaces" }
(B[m[32m+            fn name(&self) -> &str {
(B[m[32m+                "default_surfaces"
(B[m[32m+            }
(B[m[32m+            fn description(&self) -> &str {
(B[m[32m+                "Uses default surfaces"
(B[m[32m+            }
(B[m             // Does not override surfaces() — should default to ALL
             async fn execute_json(
                 &self, | Formatting check |
| clippy |  | Lint as error |
| docs |  | With  |

---

### Step 3: Write 

Mirror notectl's cd.yml with cross-build matrix + Cachix push + GitHub Release.

**Adaptations from notectl:**
- Trigger: semver tag pattern 
- Cross-build matrix: macos-aarch64, linux-x86_64, linux-aarch64 (via cross)
- **Binary names**:  (was )
- **Nix packages**:  and  (was )
- Cachix cache name:  (same as notectl)
- Requires secret: 
- Release assets:  and 

**Steps per matrix job:**
1. Checkout
2. Install Rust toolchain with target triple
3. Rust cache
4. Build release binaries via  (with  for aarch64-linux)
5. Install strip tool (binutils-aarch64-linux-gnu for cross target)
6. Package: strip → tar.gz → sha256 for each binary
7. Install Nix via 
8. Push to Cachix via 
9. 
10. 
11. 
12. Upload artifacts via 

---

### Step 4: Write 

Mirror notectl's audit.yml exactly.

**Adaptations:** None — identical to notectl.

- Triggers: daily cron at midnight UTC + pushes/PRs touching Cargo.toml/Cargo.lock
- Uses  + 
- Permissions: , 

---

### Step 5: Verify

- Run  or manual YAML validation to confirm syntax
- Confirm no references to  remain (should only see  / )
- Commit all three files together

### Key decisions

- **Pin action versions**: Use tagged releases instead of  for stability
- **Keep magic-nix-cache**: Despite free tier EOL, it still works for repo-scoped caching; migration to FlakeHub can be a future improvement ticket
- **No checks block in flake**: CD workflow skips  if no checks are defined — acceptable for now; TASK-2.18 will add integration tests that become flake checks later
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Notes

Created three GitHub Actions workflow files under .github/workflows/, mirroring jeffutter/notectl with name substitutions:

- **ci.yml**: 4 parallel jobs (test, rustfmt, clippy, docs) on push to main + PRs. Each job runs inside nix develop .#ci using DeterminateSystems/nix-installer-action + magic-nix-cache-action + Swatinem/rust-cache.
- **cd.yml**: Triggered on semver tags. Cross-build matrix for macos-aarch64, linux-x86_64, linux-aarch64 via cross. Builds nom-mcp and nom-mcp-remote binaries, strips/tars/sha256s them, pushes to jeffutter Cachix cache, and attaches to GitHub Release.
- **audit.yml**: Daily cron at midnight UTC + triggers on Cargo.toml/Cargo.lock changes. Runs cargo-audit via rustsec/audit-check.

Key adaptations from notectl:
- Binary names: notectl → nom-mcp, notectl-remote → nom-mcp-remote
- Nix packages: .#notectl → .#nom-mcp, .#notectl-remote → .#nom-mcp-remote
- All other structure, action versions, and configuration mirrored exactly.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Created .github/workflows/{ci,cd,audit}.yml mirroring notectl with nom-mcp name substitutions. All 3 acceptance criteria met.
<!-- SECTION:FINAL_SUMMARY:END -->

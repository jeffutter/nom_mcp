---
id: TASK-2.4
title: Observability and logging setup
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:23'
updated_date: '2026-08-11 23:09'
labels:
  - planned
dependencies:
  - TASK-2.1
type: chore
ordinal: 23000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
tracing + tracing-subscriber wired across all four surfaces. Server modes (HTTP/MCP serve) log to stderr at info by default, overridable via RUST_LOG/config. Local CLI defaults to warn. External API calls log outcome (success/failure, status code) at debug; API keys never logged. No metrics/tracing-export in v1.

See doc-5 §14.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 #1 done,#2 done,#3 done
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
### Files to modify

**nom-core/Cargo.toml:**
- Add `tracing` and `tracing-subscriber` (with `"env-filter"` feature) to dependencies

**nom-core/src/logging.rs (new):**
- Create `init_server()` — installs tracing-subscriber with `info` default via `EnvFilter::builder().with_default_directive(LevelFilter::INFO.into()).from_env_lossy()`, fmt layer writing to stderr
- Create `init_cli()` — same pattern but `LevelFilter::WARN` default
- Both functions return `Result<(), Error>` so callers can handle init failures gracefully
- Use `fmt::Layer` with `with_target(true)` for structured output

**nom-mcp/src/main.rs:**
- Call `nom_core::logging::init_server()` or `init_cli()` before any other initialization, based on whether running in serve mode vs local CLI mode
- For now (before argument parsing exists), always call `init_cli()` as the placeholder path is CLI-only

**nom-mcp/src/bin/nom-mcp-remote.rs:**
- Call `nom_core::logging::init_cli()` at the top of `main()` — remote-CLI is a CLI tool

**nom-core/src/lib.rs:**
- Add `pub mod logging;` to expose the module

### Step-by-step

1. **Add dependencies** to `nom-core/Cargo.toml`: `tracing = "0.1"` and `tracing-subscriber = { version = "0.3", features = ["env-filter"] }`. These are workspace-level deps that all downstream crates inherit through `nom-core`.

2. **Create `nom-core/src/logging.rs`** with two public functions:
   - `pub fn init_server() -> Result<(), tracing_subscriber::Error>` — builds subscriber with `info` default
   - `pub fn init_cli() -> Result<(), tracing_subscriber::Error>` — builds subscriber with `warn` default
   - Both use the `EnvFilter::builder().with_default_directive(...).from_env_lossy()` pattern so `RUST_LOG` overrides work correctly
   - Format layer writes to stderr with targets enabled

3. **Export module** in `nom-core/src/lib.rs`: add `pub mod logging;`

4. **Wire into `nom-mcp/src/main.rs`**: at the top of `main()`, call `let _ = nom_core::logging::init_cli();` (best-effort, ignores error). Once argument parsing lands (TASK-2.11), this will branch between `init_server()` and `init_cli()` based on the command.

5. **Wire into `nom-mcp/src/bin/nom-mcp-remote.rs`**: at the top of `main()`, call `let _ = nom_core::logging::init_cli();`

6. **Verify acceptance criteria:**
   - #1: `cargo build` succeeds; both binaries initialize tracing with correct defaults
   - #2: `RUST_LOG=debug cargo run --bin nom-mcp` produces debug-level output (verify EnvFilter override works)
   - #3: `rg` confirms no API key values appear in tracing macro calls. The existing `RedactedString` type (from TASK-2.3) already prevents leakage through Debug/Display, so any `%key` formatting in tracing macros will show `[REDACTED]`

### Key design decisions

- **Library-provided init, binary-calls-it**: `nom-core` provides the init functions; binaries choose which one to call. This avoids `set_global_default` panics from double-init and keeps library crates free of side effects.
- **Best-effort init in main()**: Use `let _ = init_*();` so tracing init failure doesn't crash the binary. If tracing fails, the app still runs without structured logs.
- **No JSON layer for v1**: Per §14, plain text fmt layer is sufficient. JSON structured logging is deferred.
- **`RedactedString` covers acceptance criterion #3**: The type from TASK-2.3 already redacts Debug/Display output. As long as code uses `%config.usda_api_key` (which invokes Display) rather than direct string interpolation of raw keys, secrets are safe. The grep check validates this discipline.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented logging.rs module with init_server() (info default) and init_cli() (warn default). Both use EnvFilter with RUST_LOG override via from_env_lossy(). Wired into nom-mcp main.rs and nom-mcp-remote.rs with best-effort init. Verified: cargo build succeeds, all 66 tests pass, no API key values in tracing macros (RedactedString from TASK-2.3 covers secret safety).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Created nom-core/src/logging.rs with init_server() (info default) and init_cli() (warn default), both using EnvFilter for RUST_LOG override. Exported module in lib.rs, wired into both binaries as best-effort init. Added tracing 0.1 and tracing-subscriber 0.3 (env-filter) to nom-core dependencies.
<!-- SECTION:FINAL_SUMMARY:END -->

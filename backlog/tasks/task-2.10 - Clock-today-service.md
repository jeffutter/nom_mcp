---
id: TASK-2.10
title: Clock / today service
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:23'
updated_date: '2026-08-12 03:11'
labels:
  - planned
dependencies:
  - TASK-2.3
  - TASK-2.7
type: feature
ordinal: 29000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Timezone resolved once at startup: explicit IANA tz from config if set, else host system-local. A single Clock owned by nom-core is injected into Operation execution and computes 'today' fresh on every call (never cached). Since the Operation registry (TASK-2.7) drives CLI/HTTP/MCP dispatch, injecting the Clock there makes all three surfaces agree on 'today' by construction. Also used at write time to materialize meals.logged_date / weight_entries.logged_date from logged_at (UTC).

See doc-5 §4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Clock resolves tz from config, falling back to system-local when unset
- [x] #2 today() is computed fresh per call, not cached at startup, and is injected into every Operation execution path (CLI, HTTP, MCP)
- [x] #3 logged_date materialization at write time uses this Clock
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan: Clock / Today Service

### Overview
Create a `Clock` type in nom-core that resolves timezone once at startup (from config or system-local), computes "today" fresh per call, and provides logged_date materialization from UTC timestamps. Injected into OperationRegistry so all three transport surfaces agree on dates by construction.

### Step 1: Add Dependencies (nom-core/Cargo.toml)

Add three crates:
- **chrono** ("0.4") — UTC time and date types (DateTime<Utc>, NaiveDate, Datelike)
- **chrono-tz** ("0.9") — IANA timezone parsing (Tz::from_str); owns bundled tzdata
- **iana-time-zone** ("0.1", optional dep) — OS-local timezone detection as fallback

Rationale over jiff: chrono+chrono-tz is the established Rust datetime stack, familiar to contributors, and integrates cleanly with existing serde patterns in the codebase. iana-time-zone provides OS-local fallback without requiring platform-specific code.

### Step 2: Create clock.rs Module (~150 lines)

File: nom-core/src/clock.rs

#### Clock struct
```rust
pub struct Clock {
    tz: chrono_tz::Tz,
}
```
Owned chrono_tz::Tz — cheap to clone (Copy). Holds the resolved timezone for the lifetime of the application.

#### Constructor — Clock::new(config: &AppConfig) -> Result<Self, ErrorData>
Resolution order:
1. If config.timezone is Some(tz_string): parse with chrono_tz::Tz::from_str(). On failure, return ErrorData::validation("timezone", "invalid IANA timezone").
2. If unset: call iana_time_zone::get_timezone(). Returns Option<String>.
   - If Some(os_tz): parse same as above
   - If None: fall back to chrono_tz::UTC with a warning via tracing::warn!("no timezone configured, using UTC")

Returns Result<Clock, ErrorData> — invalid IANA strings are validation errors.

#### Core methods
- **fn today(&self) -> chrono::NaiveDate**: Utc::now().with_timezone(&self.tz).date_naive() — zero-cache, computed fresh every call. DST-safe because UTC→TZ conversion always produces a valid local date.
- **fn logged_date(&self, utc_datetime: &DateTime<Utc>) -> chrono::NaiveDate**: utc_datetime.with_timezone(&self.tz).date_naive() — materializes logged_date from UTC logged_at at write time. Historical values are never retroactively recomputed.
- **fn format_date(d: chrono::NaiveDate) -> String**: Helper to format NaiveDate as "YYYY-MM-DD" string for SQLite storage. Uses d.format("%Y-%m-%d").to_string().

### Step 3: Wire into lib.rs (~1 line)

Add pub mod clock; to nom-core/src/lib.rs.

### Step 4: Inject Clock into OperationRegistry

File: nom-core/src/operation/registry.rs

Modify OperationRegistry to own the Clock:
```rust
pub struct OperationRegistry {
    operations: Vec<Arc<dyn Operation>>,
    clock: Arc<Clock>,
}
```

Constructor changes:
- pub fn new(clock: Arc<Clock>) -> Self — takes Clock at construction time
- Remove Default derive (clock is required)

Add accessor:
- pub fn clock(&self) -> &Clock — returns reference to the Clock

All operation lookups remain unchanged; Clock is available via registry.clock().

### Step 5: Update nom-mcp Binary Bootstrap

File: nom-mcp/src/main.rs

In execute_from_args (and future serve mode):
1. Load config: let config = AppConfig::load()?
2. Create Clock: let clock = Arc::new(Clock::new(&config)?)
3. Build registry with Clock: let mut registry = OperationRegistry::new(clock)
4. Register operations (future tasks will populate this)

### Step 6: Tests (~80 lines)

Tests in clock.rs:
- test_clock_today_returns_current_date — verify today() returns a reasonable date (within ±1 day of system date)
- test_clock_logged_date_materializes_correctly — given a known UTC DateTime, verify logged_date converts correctly for a specific TZ
- test_clock_format_date — verify format_date produces "YYYY-MM-DD" format
- test_clock_new_with_explicit_timezone — mock config with America/New_York, verify successful construction
- test_clock_new_with_invalid_timezone — mock config with Invalid/Zone, verify validation error
- test_clock_new_fallback_to_utc — mock config with no timezone, verify graceful UTC fallback

Tests in registry.rs: update existing tests to pass a Clock instance (use Clock::new() with a test config or construct directly with chrono_tz::UTC).

### Acceptance Criteria Mapping
- AC #1 (tz resolution from config/system): Step 2 constructor covers explicit IANA parsing + iana-time-zone fallback + UTC last resort
- AC #2 (fresh today(), injected into all surfaces): Step 2 today() is uncached; Step 4 injects Clock into registry which drives CLI/HTTP/MCP
- AC #3 (logged_date materialization): Step 2 logged_date() method converts UTC → local date at write time

### Risks & Edge Cases
1. DST transitions: UTC→local conversion always produces a valid date even during gaps/folds — no ambiguity since we start from UTC
2. Binary size: chrono-tz embeds tzdata (~200KB) but this is acceptable for a nutrition tracking tool
3. Test determinism: All date-sensitive tests must use fixed inputs, not rely on current time except for sanity checks
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation complete:
- Added chrono, chrono-tz, iana-time-zone dependencies to nom-core/Cargo.toml
- Created clock.rs module with Clock struct that resolves TZ from config (IANA string) → OS fallback → UTC last resort
- Clock::today() computes fresh date per call via Utc::now().with_timezone(); Clock::logged_date() materializes local date from UTC timestamp; format_date() helper for SQLite storage
- Injected Clock into OperationRegistry constructor; registry.clock() accessor provides shared access
- Updated main.rs bootstrap: loads config, creates Clock, builds registry with Clock
- Updated all tests in registry.rs, mcp_handler.rs, cli_router.rs, http_router.rs to pass Clock instances
- All 105 tests pass

Fixup applied post-review: cargo fmt --all --check failed on nom-core/src/clock.rs (two multi-line assert! calls left unwrapped) and nom-core/src/lib.rs (pub mod clock; out of alphabetical order). Ran cargo fmt --all to fix; recreates the exact CI-breaking condition TASK-6 previously fixed. See fixup commit on 16841ab.

Second fixup applied post-review: Clock::new() propagated a validation error (crashing startup) when OS-detected timezone (iana_time_zone::get_timezone() returned Ok) was a non-empty string that failed to parse as a valid IANA zone -- contradicting the function's own doc comment ('3. Last resort: fall back to UTC with a warning'), which only fired on Err(_) from get_timezone(), not on an Ok value that fails to parse. Extracted Clock::resolve_os_tz() so both OS-detection failure and OS-string-parse failure fall back to UTC with a warning, matching the documented resolution order; explicit config.timezone values still hard-fail on invalid input (that IS a user error). Added test_resolve_os_tz_falls_back_to_utc_on_unparseable_string and test_resolve_os_tz_falls_back_to_utc_on_detection_failure. See second fixup commit on 16841ab.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Created Clock/today service in nom-core with timezone resolution from config (IANA string), OS-local fallback, and UTC last resort. Clock injected into OperationRegistry so CLI/HTTP/MCP surfaces all share the same date computation. Includes logged_date materialization for write-time UTC→local conversion. All 105 tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->

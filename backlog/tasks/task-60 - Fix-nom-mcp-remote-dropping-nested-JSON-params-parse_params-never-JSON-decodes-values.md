---
id: TASK-60
title: >-
  Fix nom-mcp-remote dropping nested JSON params (parse_params never
  JSON-decodes values)
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-18 02:29'
updated_date: '2026-08-18 03:20'
labels:
  - bug
  - planned
dependencies: []
priority: high
ordinal: 66000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nom-mcp-remote cannot invoke any operation with a nested-JSON argument (log_meal/update_meal portions, create_custom_food serving_size/nutrients). Root cause: nom-core/src/cli.rs::parse_params -> parse_value only auto-types bare i64/f64/true/false and leaves everything else as a string; the value is POSTed to /api/{op} as a JSON *string*, and the operation's request deserialization fails (verified empirically: 'nom-mcp-remote log_meal portions=[{...}]' -> 400 validation 'invalid type: string "[...]", expected a sequence'). The local CLI does not have this problem because cli_router.rs::parse_value tries serde_json::from_str first, so inline JSON works there. Fix direction: make cli.rs::parse_value try serde_json::from_str before falling back to plain-string inference (parity with cli_router::parse_value), keeping the existing bare-number/bool behavior as a subset of that; add unit tests covering array/object/null passthrough plus the existing cases, and a binary-spawning e2e test (seed_data throwaway DB + serve http + nom-mcp-remote log_meal) mirroring nom-mcp/tests/seed_e2e.rs. Also remove the interim README note added by TASK-59 stating the remote CLI cannot pass nested JSON once this lands.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Root cause (verified): nom-core/src/cli.rs::parse_value — used only by nom-mcp-remote via parse_params — auto-types bare i64/f64/true/false and wraps everything else as a JSON string, so nested-JSON args (log_meal/update_meal portions, create_custom_food serving_size/nutrients) reach POST /api/{op} as a string and fail request deserialization (400 validation: invalid type: string "[...]", expected a sequence). The local CLI is unaffected because operation/cli_router.rs has its own private parse_value that tries serde_json::from_str first. The two duplicated parse_values are exactly what diverged.

Scope: single focused fix; NO sub-tickets (parser fix + unit tests + e2e test + README note are tightly coupled and ship together). Execute steps in order.

Step 1 — Fix parse_value (nom-core/src/cli.rs):
Rewrite parse_value to mirror cli_router.rs::parse_value: if let Ok(val) = serde_json::from_str(s) { return val } else { Value::String(s.to_string()) }. Update the module-level and fn doc comments: values that are valid JSON (numbers, booleans, null, arrays, objects) are sent as that JSON; anything else stays a plain string. Intentional behavior deltas vs today (all local-CLI parity): (a) existing scalar cases unchanged (42 -> number, true -> bool, 2.71 -> float); (b) "1e5" now yields integer 100000 instead of float 100000.0 — value-equivalent, and serde coerces int into f64 fields on deserialize, so no typed-request breakage; (c) double-quoted input loses its quotes ("abc" -> abc) — same as local CLI today; (d) the literal null becomes Value::Null (was string "null"); (e) non-JSON input (query=almonds, empty string) unchanged.

Step 2 — Delete the duplicated parser (nom-core/src/operation/cli_router.rs):
Remove the private parse_value there and call crate::cli::parse_value instead (it is pub). This removes the duplication whose divergence caused this bug and makes cli.rs's "shared between local-CLI and remote-CLI" module doc true again. Zero behavior change: both bodies become the identical from_str-first logic. Check cli_router.rs imports for anything left unused after removal.

Step 3 — Unit tests (cli.rs #[cfg(test)] mod tests):
All existing tests must stay green (they pin the scalar subset). Add:
- parse_value("[\"a\",\"b\"]") -> Value::Array
- parse_value("{\"k\":\"v\"}") -> Value::Object
- parse_value("null") -> Value::Null
- parse_value of the exact log_meal portions shape: [{"food_id":1,"quantity":250,"quantity_mode":"grams"}] -> Array of one Object with correct field types (i64 / f64 / string)
- parse_value("\"quoted\"") -> String("quoted") (pins the quote-stripping semantics)
- parse_params(&["portions=[{\"food_id\":1}]".into()]) round-trips an actual array through the map — the exact remote-CLI failure mode from the ticket description.

Step 4 — Binary-spawning e2e test (new file nom-mcp/tests/remote_e2e.rs), mirroring nom-mcp/tests/seed_e2e.rs conventions (TempDir, CARGO_BIN_EXE_*, env passed to children only, no std::env mutation in this process):
1. TempDir; seed throwaway DB: spawn CARGO_BIN_EXE_nom-mcp with ["seed_data", "--path", <db>] (no env override). Seed uses deterministic food ids — first food id is 1 (see nom-core/src/seed/mod.rs).
2. Free port: bind std::net::TcpListener to 127.0.0.1:0, read the assigned port, drop the listener.
3. Spawn server: CARGO_BIN_EXE_nom-mcp with ["serve", "http", "--port", <port>] and env NOM_MCP_DB_PATH=<db>; keep the Child handle.
4. Readiness probe: retry loop (~10s budget, short sleeps) running CARGO_BIN_EXE_nom-mcp-remote with ["get_goal_progress"] and env NOM_MCP_remote__server_url=http://127.0.0.1:<port> plus XDG_CONFIG_HOME=<empty temp dir> for hermeticity, until exit 0. get_goal_progress is a cheap local read op (no external API calls) and is HTTP-surfaced.
5. Act: run nom-mcp-remote log_meal with the single arg portions=[{"food_id":1,"quantity":250,"quantity_mode":"grams"}]. No shell is involved (std::process::Command), so the raw bracketed JSON reaches parse_params verbatim — this is precisely the invocation that returned 400 before the fix.
6. Assert: exit 0; stdout parses as JSON with meal_id (i64 >= 1), logged_date present, totals.calories a positive number (log_meal returns {meal_id, logged_at, logged_date, totals}).
7. Optional (include only if trivially clean): a structurally-valid-but-invalid portion (e.g. unknown food_id 999999) must exit non-zero with a rendered error — proves nested JSON now reaches server-side validation instead of dying at deserialization.
8. Cleanup: child.kill(), child.wait(), drop TempDir.

Step 5 — README cleanup (## Usage: nom-mcp-remote section):
Replace the interim note added by TASK-59 (its auto-typing is narrower: only bare numbers and true/false ... cannot be passed via the remote CLI yet (tracked in TASK-60)) with the new behavior: key=value values that are valid JSON (numbers, booleans, null, arrays, objects) are sent as that JSON, anything else as a string — same rules as the local CLI. Show a nested-JSON usage example, e.g. nom-mcp-remote log_meal portions='[{"food_id":1,"quantity":250,"quantity_mode":"grams"}]' (single-quoted so the shell passes brackets/quotes through).

Verification (matches CI): cargo fmt --all --check; cargo clippy --all-targets --all-features --workspace -- -D warnings; cargo nextest run --all-features --workspace; cargo test --doc --all-features --workspace. If practical, confirm red/green: the new unit + e2e assertions fail against the pre-fix parse_value and pass after.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation (all plan steps executed in order):
(1) nom-core/src/cli.rs::parse_value rewritten to try serde_json::from_str first, falling back to Value::String — exact parity with the local-CLI router. Module + fn doc comments updated to describe the JSON-first rule. Scalar subset unchanged (verified: pre-existing number/float/bool/string tests stay green).
(2) Deleted the private duplicated parse_value in nom-core/src/operation/cli_router.rs; dispatch now calls crate::cli::parse_value (pub). No import changes needed (serde_json paths there are fully qualified and still used).
(3) New unit tests in cli.rs: array/object/null passthrough, exact log_meal portions shape (field types i64/f64/string), quoted-string quote-stripping pin, and parse_params round-trip of 'portions=[{"food_id":1}]' (the exact remote-CLI failure mode). All pre-existing tests untouched and green.
(4) New e2e test nom-mcp/tests/remote_e2e.rs (mirrors seed_e2e.rs conventions): seeds throwaway DB via CARGO_BIN_EXE_nom-mcp seed_data, grabs a free port via TcpListener bind, spawns real 'serve http' child with NOM_MCP_DB_PATH + empty XDG_CONFIG_HOME (hermetic), readiness-probes via nom-mcp-remote get_goal_progress (~10s budget), then runs 'nom-mcp-remote log_meal portions=[{...}]' verbatim (no shell) and asserts exit 0 + {meal_id>=1, logged_date non-empty, totals.total_calories>0}. Also includes the optional negative case: unknown food_id 999999 exits non-zero with rendered stderr error (proves nested JSON reaches server-side validation). ServerGuard Drop kills+reaps the server on panic paths too.
(5) README 'Usage: nom-mcp-remote' section: removed the TASK-59 interim note about narrow auto-typing; documents the JSON-first value rule (same as local CLI) and adds a single-quoted nested-JSON log_meal example.
Verification (CI-parity): cargo fmt --all --check OK; clippy -D warnings OK; nextest 328/328 pass (incl. new unit + e2e); doctests OK; rustdoc -D warnings OK. Red/green confirmed: temporarily reverting parse_value to the old body makes all 5 new assertions fail while the scalar-subset tests stay green; restoring the fix turns them green.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed: nom-mcp-remote now passes nested-JSON args correctly. cli::parse_value (shared by local-CLI router and remote-CLI) JSON-decodes values first — valid JSON (numbers, booleans, null, arrays, objects) is sent as that JSON, anything else as a string — so 'nom-mcp-remote log_meal portions=[{"food_id":1,...}]' works end-to-end against a real serve http server (verified by new binary-spawning e2e test). The duplicated cli_router parse_value was deleted in favor of the shared one, removing the divergence that caused this bug. README interim note from TASK-59 replaced with the new behavior + example. No acceptance criteria were defined on this ticket; the implementation plan's 5 steps + verification are all complete and recorded in notes.
<!-- SECTION:FINAL_SUMMARY:END -->

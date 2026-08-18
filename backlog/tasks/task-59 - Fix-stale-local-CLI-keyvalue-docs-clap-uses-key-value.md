---
id: TASK-59
title: Fix stale local-CLI key=value docs (clap uses --key value)
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-18 02:08'
updated_date: '2026-08-18 02:49'
labels:
  - docs
  - cli
  - planned
dependencies: []
priority: medium
type: task
ordinal: 65000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-54 execution found that README.md and AGENTS.md document the local CLI as 'nom-mcp <operation> key=value ...', but since TASK-30 the local CLI is clap-backed and only accepts '--key value' (verified empirically: 'nom-mcp log_weight value=79.9' fails with 'unexpected argument'; 'nom-mcp log_weight --value 79.9' works). nom-core/src/cli.rs::parse_params (true key=value) is only used by nom-mcp-remote, which genuinely is key=value. Affected doc sites: README.md 'The four surfaces' table (local CLI + remote CLI rows), 'Usage: local CLI' intro + all operation examples, 'REST API' section ('matching the operation key=value arguments'), 'Usage: nom-mcp-remote' (correct there), and AGENTS.md Commands section ('cargo run -p nom-mcp --bin nom-mcp -- <operation> key=value ...'). Decide whether to also make the local CLI accept key=value for parity with the remote CLI (would need cli_router to fall back to parse_params when no subcommand matches, or per-arg handling) — if not, fix the docs to show --key value.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 README.md local-CLI sections show the actual working invocation syntax (--key value) for every documented example
- [x] #2 AGENTS.md Commands section shows the actual working local-CLI invocation syntax
- [x] #3 Either the local CLI accepts key=value like nom-mcp-remote (with tests), or the docs explicitly note the two CLIs differ in arg syntax
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Approach: docs-only fix (ticket path #1). Decision rationale: (a) AC#3 explicitly allows 'docs explicitly note the two CLIs differ'; (b) clap already accepts both '--key value' and '--key=value', so the only missing form is bare 'key=value', which a fallback parser would have to disambiguate against subcommand names — adding a second parsing path to a stable v1 surface for marginal ergonomics gain; (c) the deeper divergence found during planning (remote CLI cannot pass nested JSON params at all) is a functional bug, filed separately as TASK-60, not a syntax-parity question. No code changes in this ticket.

Verified ground truth (empirical, 2026-08-18, built binaries + seeded throwaway DB): local CLI 'nom-mcp log_weight --value 79.9' OK; '--value=79.9' OK; 'value=79.9' rejected with 'unexpected argument'. Local CLI values go through cli_router.rs::parse_value which tries serde_json::from_str first, so inline single-quoted JSON works for nested fields (verified create_custom_food with --serving_size/--nutrients JSON strings). Remote CLI 'nom-mcp-remote log_weight value=79.9' OK; 'log_meal portions=[...]' fails (tracked in TASK-60).

Edits — README.md:
1. 'The four surfaces' table, Local CLI row: 'nom-mcp <operation> key=value ...' -> 'nom-mcp <operation> --key value ...'.
2. Same table, Remote CLI row: drop 'Same CLI ergonomics as the local CLI' (syntax differs); say it takes bare 'key=value' args instead of the local CLI's '--key value' flags, talks to serve http REST API.
3. 'Usage: local CLI' code block: 'nom-mcp <operation> [key=value ...]' -> 'nom-mcp <operation> [--key value ...]'.
4. Following paragraph: rewrite arg description — arguments are '--key value' long flags (clap also accepts '--key=value'); values auto-typed by trying JSON parse first (numbers/booleans/null/'[...]'/ '{...}' become JSON, everything else a string), so nested-JSON fields can be passed inline as single-quoted JSON; keep the '--help' sentence; ADD explicit note that nom-mcp-remote uses bare 'key=value' pairs instead (satisfies AC#3).
5. Operations table: convert every row's key=value shorthand to flag form: search_food --query <text or barcode>; create_custom_food --name <text> --serving_size <json> --nutrients <json>; log_meal --portions <json>; update_meal --meal_id <id> ...; delete_meal --meal_id <id>; search_meals --query <text>; get_meals_by_date_range --start <date> --end <date>; log_weight --value <number>; update_weight_entry --id <id> ...; delete_weight_entry --id <id>; get_weight_by_date --date <date>; get_weight_by_date_range --start <date> --end <date>; set_nutrition_goals --calories <n> --calories_direction <target|minimum|maximum> ...; get_goal_progress --date <date>. get_weight_today unchanged.
6. Example block: convert to flag form exactly as written there (search_food --query almonds; log_meal --portions '[{...}]'; create_custom_food --name "Protein Shake" --serving_size '{...}' --nutrients '{...}'; log_weight --value 181.4; get_weight_by_date_range --start 2026-08-01 --end 2026-08-12; set_nutrition_goals --calories 2200 --calories_direction target --protein_g 150 --protein_g_direction minimum; get_goal_progress).
7. REST API section: 'with a JSON request body matching the operation's key=value arguments' -> 'with a JSON object whose keys are the operation's argument names' (REST bodies are real JSON, never key=value strings).
8. 'Usage: nom-mcp-remote' section: reword 'same CLI surface as the local CLI' to 'exposes the same operations ... but takes bare key=value arguments'; examples stay as-is (verified working); add one accurate sentence on remote auto-typing (bare numbers/true/false only) plus interim note that nested-JSON args (e.g. log_meal portions) cannot be passed via the remote CLI yet — tracked in TASK-60. Remove that interim sentence when TASK-60 lands.

Edits — AGENTS.md: Commands section line 'cargo run -p nom-mcp --bin nom-mcp -- <operation> key=value ...' -> '... -- <operation> --key value ...'. (Only local-CLI doc site in AGENTS.md; confirmed by repo-wide grep.)

Verification:
- Re-run every converted example from the README example block (and the ops-table forms) against a fresh throwaway seeded DB: 'nom-mcp seed_data --path /tmp/nom-dev-task59/nom.db' then each command with NOM_MCP_DB_PATH set; all must succeed. Also confirm 'nom-mcp log_weight value=79.9' still errors (documents the rejection correctly).
- Grep README.md + AGENTS.md for 'key=value': remaining hits must all be in remote-CLI/REST-context sentences only; zero in local-CLI context.
- Docs-only change: no cargo gates needed beyond a final 'cargo fmt --all --check' sanity pass (no Rust touched).

AC mapping: #1 = edits 1-7 (every local-CLI example shows verified-working '--key value' syntax); #2 = AGENTS.md edit; #3 = edit 4's explicit difference note (+ edit 2/8).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Executed per plan (docs-only, no Rust touched). All 8 planned README edits + the AGENTS.md Commands-section edit applied. Plan corrections found during empirical verification (built debug binaries, seeded throwaway DBs via seed_data): (1) the plan's '--start <date> --end <date>' for get_meals_by_date_range/get_weight_by_date_range is wrong — real clap flags are --start_date/--end_date (verified via --help and by running); (2) 'update_weight_entry/delete_weight_entry --id' is wrong — real flag is --entry_id; (3) get_goal_progress date is optional, rendered as '[--date <date>]'. Every converted example from the README example block was re-run verbatim against a fresh seeded throwaway DB (all OK), plus update_meal/delete_meal/search_meals/get_weight_today/get_weight_by_date/update_weight_entry/delete_weight_entry/get_meals_by_date_range individually verified. 'nom-mcp log_weight value=79.9' still rejected with 'unexpected argument' (docs now describe this correctly). Grep gate: remaining key=value hits in README.md are all remote-CLI-context only (four-surfaces table row, the explicit difference note, nom-mcp-remote section); zero local-CLI context; AGENTS.md clean. cargo fmt --all --check passes. AC#3 satisfied via the explicit difference notes (edit 4 + edit 8), not a parser change. Committed the untracked TASK-60 ticket file alongside since the README's interim note references it.
<!-- SECTION:NOTES:END -->

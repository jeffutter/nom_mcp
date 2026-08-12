---
id: TASK-17
title: >-
  Fix: remove unreachable!() panic in SearchMeals's hand-rolled parameter-count
  dispatch
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-12 20:22'
updated_date: '2026-08-12 22:29'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-2.14
priority: high
ordinal: 190
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.14 (nom-core/src/meal/mod.rs:1336, inside SearchMeals's match params.len() { 1 => ..., 2 => ..., 3 => ..., _ => unreachable!() } dispatch spanning roughly lines 1260-1337). This works around dynamic SQL parameter binding by hand-writing three nearly-identical match arms (one per possible bound-parameter count from the optional date_range fields) and falling back to unreachable!() for any other count. Currently unreachable given the two optional date-range fields, but it is a live panic macro in non-test code -- the project's convention (see CLAUDE.md 'errors as values, no panics across the boundary') forbids unwrap/expect/panic outside tests, and this is exactly the kind of latent landmine that convention exists to prevent: a future added optional search filter that changes the possible param counts would make this reachable and crash the MCP server process instead of returning an error. It is also copy-pasted boilerplate (three near-identical query-and-collect blocks) rather than a single dynamic-params bind call.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The three near-identical match arms (params.len() == 1, 2, 3) in SearchMeals::execute_json are replaced with a single code path that binds a dynamic-length parameter list to the query, without any unreachable!()/panic!()/unwrap()/expect() in the non-test path
- [x] #2 If the turso crate's query API requires a fixed-arity tuple for binding (check its docs/existing usage in this file), the replacement uses whatever dynamic-binding mechanism it provides (e.g. binding a Vec of turso::Value, or building the SQL string with the exact right number of ? placeholders and binding a slice) instead of a match-per-arity; if no such mechanism exists, the fallback arm returns Err(ErrorData::storage_failure(...)) instead of unreachable!(), and a comment explains why the match is exhaustive-in-practice
- [x] #3 Existing search_meals tests (test_search_meals_matches_food_names, test_search_meals_no_results, and any date_range-filtered search test) pass unchanged
- [x] #4 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan\n\n### Context\nSearchMeals::execute_json builds SQL dynamically (adding WHERE clauses for optional date_range.start/end) and a matching . Currently dispatches via  with three near-identical query-and-collect blocks.\n\n### Root Cause\nThe hand-rolled match exists because the author assumed turso required fixed-arity tuples. However, turso 0.8.0-pre.4 implements  for  where , and  — so  works natively.\n\n### Change (single file: nom-core/src/meal/mod.rs)\n\n**Replace lines ~1340-1404** (the entire  block including the  arm):\n\n\n\n### Key Details\n-  passes ownership to  — no clone needed since params isn't used after this point\n- The  call already exists before the match — keep it\n- Remove the match entirely; one code path handles all param counts (1, 2, or 3)\n- No panic macros remain in non-test paths\n\n### Verification\n1. Run existing tests: \n2. Run full test suite: 
running 154 tests
test client::off::tests::test_client_new_rejects_non_base_url ... ok
test client::off::tests::test_deserialize_not_found ... ok
test client::off::tests::test_empty_product_defaults ... ok
test client::off::tests::test_deserialize_partial_response ... ok
test client::off::tests::test_deserialize_full_response ... ok
test client::off::tests::test_client_with_default_base ... ok
test client::off::tests::test_client_new_sets_user_agent ... ok
test client::usda::tests::test_client_invalid_url ... ok
test client::off::tests::test_missing_nutriments_defaults ... ok
test client::usda::tests::test_client_new ... ok
test client::usda::tests::test_client_with_default_base ... ok
test client::usda::tests::test_deserialize_minimal_search_response ... ok
test client::usda::tests::test_deserialize_batch_response ... ok
test client::usda::tests::test_deserialize_empty_detail_response ... ok
test client::usda::tests::test_extract_macros_from_full_nutrients ... ok
test client::usda::tests::test_extract_macros_partial ... ok
test client::usda::tests::test_deserialize_partial_detail_response ... ok
test client::usda::tests::test_deserialize_full_detail_response ... ok
test client::usda::tests::test_extract_macros_empty ... ok
test client::off::tests::test_lookup_barcode_network_error ... ok
test client::usda::tests::test_deserialize_full_search_response ... ok
test client::usda::tests::test_portion_info_defaults ... ok
test client::usda::tests::test_portion_info ... ok
test clock::tests::test_clock_logged_date_materializes_correctly ... ok
test clock::tests::test_clock_format_date ... ok
test clock::tests::test_clock_today_returns_reasonable_date ... ok
test clock::tests::test_resolve_os_tz_falls_back_to_utc_on_detection_failure ... ok
test clock::tests::test_resolve_os_tz_falls_back_to_utc_on_unparseable_string ... ok
test clock::tests::test_resolve_os_tz_uses_valid_os_string ... ok
test config::tests::test_default_http_bind_address ... ok
test config::tests::test_db_path_creates_parent_directory ... ok
test config::tests::test_default_off_user_agent_contains_version ... ok
test clock::tests::test_clock_new_with_explicit_timezone ... ok
test clock::tests::test_clock_new_with_invalid_timezone ... ok
test clock::tests::test_clock_new_fallback_to_utc ... ok
test config::tests::test_redacted_debug_output ... ok
test config::tests::test_redacted_deserialization ... ok
test config::tests::test_redacted_display_output ... ok
test config::tests::test_redacted_get_returns_actual_value ... ok
test config::tests::test_redacted_serialization ... ok
test client::usda::tests::test_network_error_propagates ... ok
test client::usda::tests::test_get_foods_batch_empty ... ok
test client::off::tests::test_user_agent_header_reaches_server ... ok
test client::usda::tests::test_api_error_on_500 ... ok
test error::tests::test_conflict_serialization ... ok
test error::tests::test_deserialize_minimal_json ... ok
test error::tests::test_exit_code_mapping ... ok
test error::tests::test_external_api_failure_serialization ... ok
test error::tests::test_format_success_pretty_prints_json ... ok
test error::tests::test_http_status_mapping ... ok
test error::tests::test_not_found_serialization ... ok
test client::off::tests::test_lookup_barcode_not_found ... ok
test client::usda::tests::test_api_key_appears_as_query_param ... ok
test error::tests::test_render_conflict ... ok
test client::off::tests::test_lookup_barcode_success ... ok
test client::off::tests::test_lookup_barcode_normalizes_barcode ... ok
test error::tests::test_render_lock_probe ... ok
test error::tests::test_render_external_api_failure ... ok
test error::tests::test_render_not_found ... ok
test error::tests::test_render_storage_failure ... ok
test error::tests::test_round_trip_serialization ... ok
test error::tests::test_render_validation ... ok
test client::off::tests::test_lookup_barcode_query_injection_prevented ... ok
test food::tests::test_convert_to_per_100g_basic ... ok
test error::tests::test_storage_failure_serialization ... ok
test error::tests::test_validation_serialization ... ok
test client::off::tests::test_lookup_barcode_unexpected_status ... ok
test client::usda::tests::test_search_foods_pagination ... ok
test food::tests::test_convert_to_per_100g_zero_serving ... ok
test food::tests::test_extract_off_macros_defaults_to_zero_when_missing ... ok
test food::tests::test_extract_off_macros_prefers_100g_fields ... ok
test food::tests::test_extract_off_macros_no_nutriments ... ok
test food::tests::test_is_barcode_digit_only ... ok
test food::tests::test_is_barcode_empty ... ok
test client::off::tests::test_lookup_barcode_injection_prevented ... ok
test food::tests::test_is_barcode_rejects_non_digits ... ok
test food::tests::test_merge_candidates_custom_first_and_cap ... ok
test logging::tests::test_build_filter_cli_default ... ok
test logging::tests::test_build_filter_server_default ... ok
test meal::tests::test_compute_portion_macros_grams_mode ... ok
test meal::tests::test_compute_portion_macros_servings_mode ... ok
test meal::tests::test_compute_portion_macros_servings_no_serving_size ... ok
test meal::tests::test_compute_totals_basic ... ok
test meal::tests::test_compute_totals_with_adjustment ... ok
test config::tests::test_env_overrides_toml ... ok
test client::usda::tests::test_get_food_success ... ok
test config::tests::test_load_with_no_config_file_or_env ... ok
test config::tests::test_missing_config_file_is_not_an_error ... ok
test client::usda::tests::test_get_food_not_found ... ok
test client::usda::tests::test_get_foods_batch_success ... ok
test client::usda::tests::test_url_no_double_slash_with_bare_origin ... ok
test client::usda::tests::test_rate_limited_returns_error ... ok
test client::usda::tests::test_search_foods_success ... ok
test config::tests::test_toml_overrides_defaults ... ok
test config::tests::test_usda_key_is_redacted_in_debug ... ok
test client::usda::tests::test_get_foods_batch_auto_chunking ... ok
test food::tests::test_create_custom_food_rejects_negative_serving ... ok
test food::tests::test_create_custom_food_accepts_gram_aliases ... ok
test food::tests::test_create_custom_food_rejects_non_gram_unit ... ok
test food::tests::test_create_custom_food_rejects_zero_serving ... ok
test food::tests::test_create_custom_food_stores_per_100g ... ok
test operation::cli_router::tests::test_build_cli_command_includes_cli_ops ... ok
test operation::cli_router::tests::test_parse_and_dispatch_no_subcommand ... ok
test operation::http_router::tests::test_build_http_router_has_routes ... ok
test operation::http_router::tests::test_handle_operation_error_serializes_error_data_body ... ok
test operation::mcp_handler::tests::test_bad_schema_does_not_panic ... ok
test operation::mcp_handler::tests::test_empty_registry_list_tools ... ok
test operation::mcp_handler::tests::test_get_tool_skips_bad_schema ... ok
test operation::mcp_handler::tests::test_list_tools_omits_bad_schema_but_keeps_good_ops ... ok
test operation::mcp_handler::tests::test_mcp_handler_new ... ok
test operation::mcp_handler::tests::test_tool_from_operation_has_required_fields ... ok
test operation::registry::tests::test_clock_accessor ... ok
test operation::registry::tests::test_default_surfaces_is_all ... ok
test operation::registry::tests::test_empty_registry ... ok
test operation::registry::tests::test_filter_by_cli_surface ... ok
test operation::registry::tests::test_filter_by_http_surface ... ok
test operation::registry::tests::test_filter_by_mcp_surface ... ok
test operation::registry::tests::test_register_and_get ... ok
test operation::tests::test_surfaces_cli_only ... ok
test operation::tests::test_surfaces_default_is_all ... ok
test operation::tests::test_surfaces_http_only ... ok
test operation::tests::test_surfaces_intersection ... ok
test operation::tests::test_surfaces_mcp_only ... ok
test food::tests::test_search_food_free_text_custom_only ... ok
test storage::lock_probe::tests::test_probe_missing_file ... ok
test storage::lock_probe::tests::test_probe_unlocked_file ... ok
test storage::test::test_all_six_tables_created ... ok
test food::tests::test_search_food_barcode_success ... ok
test storage::test::test_indexes_exist ... ok
test food::tests::test_search_food_barcode_not_found ... ok
test storage::test::test_fk_enforcement_active ... ok
test storage::test::test_migrations_table_has_version_entry ... ok
test storage::test::test_migration_idempotency ... ok
test food::tests::test_search_food_upsert_idempotency ... ok
test food::tests::test_search_food_free_text_usda_merge ... ok
test logging::tests::test_rust_log_override ... ok
test meal::tests::test_delete_meal_cascades_to_portions ... ok
test meal::tests::test_delete_meal_not_found_error ... ok
test storage::lock_probe::tests::test_probe_locked_file ... ok
test meal::tests::test_get_meals_by_date_range ... ok
test meal::tests::test_get_meals_by_date_range_empty ... ok
test meal::tests::test_log_meal_creates_meal_and_portions_with_snapshots ... ok
test meal::tests::test_log_meal_materializes_totals_correctly ... ok
test meal::tests::test_log_meal_servings_mode ... ok
test meal::tests::test_log_meal_rejects_empty_portions ... ok
test meal::tests::test_log_meal_rejects_zero_quantity ... ok
test meal::tests::test_log_meal_validates_food_id_not_found ... ok
test meal::tests::test_search_meals_matches_food_names ... ok
test meal::tests::test_search_meals_no_results ... ok
test meal::tests::test_snapshot_semantics_editing_uses_own_snapshot ... ok
test meal::tests::test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change ... ok
test meal::tests::test_update_meal_full_portion_replacement ... ok
test meal::tests::test_update_meal_not_found_error ... ok
test meal::tests::test_update_meal_partial_patch_adjustment_only ... ok

test result: ok. 154 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.06s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n3. Run clippy: \n4. Confirm no unreachable!/panic!/unwrap()/expect() remain in non-test SearchMeals code\n
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Replaced match params.len() { 1 => ..., 2 => ..., 3 => ..., _ => unreachable!() } with a single stmt.query(params) call leveraging turso's Vec<T: IntoValue> impl of IntoParams. Removed 58 lines of copy-pasted boilerplate, replaced with 16 lines. No panic macros remain in non-test paths.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Eliminated unreachable!() panic in SearchMeals by replacing hand-rolled match-per-arity dispatch with turso's native Vec<T> dynamic parameter binding. Single code path handles all parameter counts (1-3) without copy-pasted boilerplate or panic macros.
<!-- SECTION:FINAL_SUMMARY:END -->

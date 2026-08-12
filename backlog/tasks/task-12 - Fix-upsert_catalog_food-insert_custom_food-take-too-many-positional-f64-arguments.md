---
id: TASK-12
title: >-
  Fix: upsert_catalog_food/insert_custom_food take too many positional f64
  arguments
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-12 05:29'
updated_date: '2026-08-12 21:35'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-2.13
priority: high
ordinal: 160
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.13 (nom-core/src/food/mod.rs:86-149 upsert_catalog_food, 10 args; :155-203 insert_custom_food, 8 args). Both functions take calories/protein/carbs/fat/fiber as five consecutive same-typed f64 positional parameters (plus source/external_id/name as consecutive &str params on upsert_catalog_food), flagged by clippy::too_many_arguments (nix develop -c cargo clippy --workspace --all-targets: 'this function has too many arguments (10/7)' and '(8/7)'). All current call sites happen to pass arguments in the correct order, but this shape is a latent transposition hazard per the project's 'General-Purpose Interfaces' design philosophy (CLAUDE.md) — swapping fat and fiber, or carbs and protein, at a future call site would compile silently and corrupt stored nutrition data with no error. FoodCandidate already has the right shape (a struct with named fields) for this data; these two DB-helper functions should take the same kind of grouped value instead of five loose f64s.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 upsert_catalog_food and insert_custom_food each take a single grouped nutrients parameter (e.g. a shared NutrientValues { calories, protein_g, carbs_g, fat_g, fiber_g } struct, or reuse/derive from FoodCandidate's fields) instead of 5 individual f64 parameters
- [x] #2 nix develop -c cargo clippy --workspace --all-targets no longer reports too_many_arguments for these two functions
- [x] #3 all existing call sites in food/mod.rs are updated and nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

### Step 1: Add NutrientValues struct (~line 45, after FoodCandidate)

Add a small Copy-able struct near the top of the shared types section:

-  so it's accessible within the crate but not part of the public API
-  derived (all fields are ) — passed by value, zero allocation overhead
- Field names match the existing  struct convention (, etc.) for consistency

### Step 2: Change extract_off_macros() to return NutrientValues (~line 239)

Change signature from  to :

This eliminates the tuple unpacking at the call site and provides semantic labels at the source.

### Step 3: Update upsert_catalog_food signature (~line 86)

Replace five individual  params with single :

BEFORE (10 args):

AFTER (7 args):

Update the SQL binding tuple to use , , etc.

### Step 4: Update insert_custom_food signature (~line 155)

Same pattern — replace five  params with :

BEFORE (8 args):

AFTER (5 args):

Update the SQL binding tuple similarly.

### Step 5: Update call sites

**Call site 1 — search_barcode (~line 427):**

Also update the tuple destructure  to use the struct directly.

**Call site 2 — search_free_text USDA branch (~line 493):**

**Call site 3 — CreateCustomFood::execute_json (~line 661):**

### Step 6: Update tests that call extract_off_macros

Three unit tests destructuring the tuple — change from:

to:

### Step 7: Verify

1.  — confirm too_many_arguments gone for these two functions
2. 
running 153 tests
test client::off::tests::test_client_new_rejects_non_base_url ... ok
test client::off::tests::test_empty_product_defaults ... ok
test client::off::tests::test_deserialize_full_response ... ok
test client::off::tests::test_deserialize_not_found ... ok
test client::off::tests::test_deserialize_partial_response ... ok
test client::off::tests::test_client_new_sets_user_agent ... ok
test client::off::tests::test_client_with_default_base ... ok
test client::off::tests::test_missing_nutriments_defaults ... ok
test client::usda::tests::test_client_invalid_url ... ok
test client::usda::tests::test_deserialize_batch_response ... ok
test client::usda::tests::test_deserialize_full_search_response ... ok
test client::usda::tests::test_extract_macros_empty ... ok
test client::usda::tests::test_extract_macros_from_full_nutrients ... ok
test client::usda::tests::test_extract_macros_partial ... ok
test client::usda::tests::test_deserialize_full_detail_response ... ok
test client::usda::tests::test_client_new ... ok
test client::usda::tests::test_client_with_default_base ... ok
test client::off::tests::test_lookup_barcode_network_error ... ok
test client::usda::tests::test_deserialize_minimal_search_response ... ok
test client::usda::tests::test_deserialize_partial_detail_response ... ok
test client::usda::tests::test_deserialize_empty_detail_response ... ok
test clock::tests::test_clock_format_date ... ok
test client::usda::tests::test_portion_info ... ok
test client::usda::tests::test_portion_info_defaults ... ok
test clock::tests::test_clock_logged_date_materializes_correctly ... ok
test clock::tests::test_clock_today_returns_reasonable_date ... ok
test clock::tests::test_resolve_os_tz_falls_back_to_utc_on_detection_failure ... ok
test clock::tests::test_resolve_os_tz_falls_back_to_utc_on_unparseable_string ... ok
test clock::tests::test_resolve_os_tz_uses_valid_os_string ... ok
test config::tests::test_db_path_creates_parent_directory ... ok
test config::tests::test_default_http_bind_address ... ok
test clock::tests::test_clock_new_fallback_to_utc ... ok
test clock::tests::test_clock_new_with_explicit_timezone ... ok
test clock::tests::test_clock_new_with_invalid_timezone ... ok
test config::tests::test_default_off_user_agent_contains_version ... ok
test config::tests::test_redacted_debug_output ... ok
test config::tests::test_redacted_deserialization ... ok
test config::tests::test_redacted_display_output ... ok
test config::tests::test_redacted_get_returns_actual_value ... ok
test config::tests::test_redacted_serialization ... ok
test client::usda::tests::test_network_error_propagates ... ok
test client::usda::tests::test_get_foods_batch_empty ... ok
test client::off::tests::test_lookup_barcode_success ... ok
test client::usda::tests::test_api_key_appears_as_query_param ... ok
test config::tests::test_env_overrides_toml ... ok
test error::tests::test_conflict_serialization ... ok
test error::tests::test_exit_code_mapping ... ok
test error::tests::test_external_api_failure_serialization ... ok
test error::tests::test_deserialize_minimal_json ... ok
test client::off::tests::test_lookup_barcode_normalizes_barcode ... ok
test client::usda::tests::test_api_error_on_500 ... ok
test client::off::tests::test_lookup_barcode_unexpected_status ... ok
test client::off::tests::test_lookup_barcode_not_found ... ok
test client::off::tests::test_lookup_barcode_query_injection_prevented ... ok
test error::tests::test_http_status_mapping ... ok
test error::tests::test_render_conflict ... ok
test config::tests::test_load_with_no_config_file_or_env ... ok
test error::tests::test_render_external_api_failure ... ok
test error::tests::test_render_not_found ... ok
test error::tests::test_not_found_serialization ... ok
test error::tests::test_render_lock_probe ... ok
test client::off::tests::test_lookup_barcode_injection_prevented ... ok
test error::tests::test_format_success_pretty_prints_json ... ok
test error::tests::test_render_storage_failure ... ok
test error::tests::test_render_validation ... ok
test config::tests::test_missing_config_file_is_not_an_error ... ok
test error::tests::test_round_trip_serialization ... ok
test error::tests::test_storage_failure_serialization ... ok
test error::tests::test_validation_serialization ... ok
test client::usda::tests::test_rate_limited_returns_error ... ok
test food::tests::test_convert_to_per_100g_zero_serving ... ok
test food::tests::test_convert_to_per_100g_basic ... ok
test client::usda::tests::test_get_foods_batch_success ... ok
test food::tests::test_extract_off_macros_prefers_100g_fields ... ok
test food::tests::test_extract_off_macros_defaults_to_zero_when_missing ... ok
test food::tests::test_is_barcode_digit_only ... ok
test food::tests::test_is_barcode_empty ... ok
test client::usda::tests::test_get_food_not_found ... ok
test food::tests::test_extract_off_macros_no_nutriments ... ok
test food::tests::test_is_barcode_rejects_non_digits ... ok
test food::tests::test_merge_candidates_custom_first_and_cap ... ok
test client::off::tests::test_user_agent_header_reaches_server ... ok
test meal::tests::test_compute_portion_macros_grams_mode ... ok
test logging::tests::test_build_filter_server_default ... ok
test logging::tests::test_build_filter_cli_default ... ok
test meal::tests::test_compute_portion_macros_servings_no_serving_size ... ok
test meal::tests::test_compute_portion_macros_servings_mode ... ok
test client::usda::tests::test_url_no_double_slash_with_bare_origin ... ok
test meal::tests::test_compute_totals_basic ... ok
test meal::tests::test_compute_totals_with_adjustment ... ok
test client::usda::tests::test_search_foods_pagination ... ok
test client::usda::tests::test_get_food_success ... ok
test client::usda::tests::test_search_foods_success ... ok
test config::tests::test_toml_overrides_defaults ... ok
test client::usda::tests::test_get_foods_batch_auto_chunking ... ok
test config::tests::test_usda_key_is_redacted_in_debug ... ok
test food::tests::test_create_custom_food_accepts_gram_aliases ... ok
test food::tests::test_create_custom_food_rejects_non_gram_unit ... ok
test food::tests::test_create_custom_food_stores_per_100g ... ok
test food::tests::test_create_custom_food_rejects_zero_serving ... ok
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
test food::tests::test_create_custom_food_rejects_negative_serving ... ok
test storage::lock_probe::tests::test_probe_missing_file ... ok
test storage::lock_probe::tests::test_probe_unlocked_file ... ok
test food::tests::test_search_food_free_text_usda_merge ... ok
test storage::test::test_all_six_tables_created ... ok
test logging::tests::test_rust_log_override ... ok
test food::tests::test_search_food_free_text_custom_only ... ok
test storage::test::test_indexes_exist ... ok
test storage::test::test_fk_enforcement_active ... ok
test storage::test::test_migrations_table_has_version_entry ... ok
test food::tests::test_search_food_barcode_not_found ... ok
test storage::test::test_migration_idempotency ... ok
test food::tests::test_search_food_barcode_success ... ok
test meal::tests::test_delete_meal_cascades_to_portions ... ok
test meal::tests::test_get_meals_by_date_range ... ok
test meal::tests::test_delete_meal_not_found_error ... ok
test meal::tests::test_log_meal_servings_mode ... ok
test meal::tests::test_log_meal_rejects_zero_quantity ... ok
test meal::tests::test_log_meal_validates_food_id_not_found ... ok
test meal::tests::test_search_meals_matches_food_names ... ok
test meal::tests::test_search_meals_no_results ... ok
test meal::tests::test_snapshot_semantics_editing_uses_own_snapshot ... ok
test meal::tests::test_log_meal_creates_meal_and_portions_with_snapshots ... ok
test meal::tests::test_log_meal_rejects_empty_portions ... ok
test meal::tests::test_log_meal_materializes_totals_correctly ... ok
test food::tests::test_search_food_upsert_idempotency ... ok
test meal::tests::test_get_meals_by_date_range_empty ... ok
test meal::tests::test_update_meal_full_portion_replacement ... ok
test meal::tests::test_update_meal_not_found_error ... ok
test meal::tests::test_update_meal_partial_patch_adjustment_only ... ok
test storage::lock_probe::tests::test_probe_locked_file ... ok

test result: ok. 153 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.48s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s — all tests pass
3. Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:857:
             // Update logged_at if provided
             if let Some(ref ts) = req.logged_at {
                 let dt: DateTime<Utc> = ts.parse().map_err(|_| {
[31m-                    ErrorData::validation(
(B[m[31m-                        "logged_at",
(B[m[31m-                        format!("invalid datetime format: {}", ts),
(B[m[31m-                    )
(B[m[32m+                    ErrorData::validation("logged_at", format!("invalid datetime format: {}", ts))
(B[m                 })?;
                 let logged_at_str = format!("{}", dt.format("%Y-%m-%dT%H:%M:%SZ"));
                 let logged_date_str = Clock::format_date(self.clock.logged_date(&dt));
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:892:
             // Replace portions if provided (full replacement semantics)
             if let Some(new_portions) = &req.portions {
                 // Delete old portions
[31m-                conn.execute(
(B[m[31m-                    "DELETE FROM portions WHERE meal_id = ?",
(B[m[31m-                    (req.meal_id,),
(B[m[31m-                )
(B[m[31m-                .await
(B[m[31m-                .map_err(|e| ErrorData::storage_failure(format!("delete portions failed: {e}")))?;
(B[m[32m+                conn.execute("DELETE FROM portions WHERE meal_id = ?", (req.meal_id,))
(B[m[32m+                    .await
(B[m[32m+                    .map_err(|e| {
(B[m[32m+                        ErrorData::storage_failure(format!("delete portions failed: {e}"))
(B[m[32m+                    })?;
(B[m 
                 let mut all_macros: Vec<(f64, f64, f64, f64, f64)> = Vec::new();
[31m-                let mut snapshots: Vec<(i64, &str, f64, f64, f64, f64, f64, f64, Option<f64>)> = Vec::new();
(B[m[32m+                let mut snapshots: Vec<(i64, &str, f64, f64, f64, f64, f64, f64, Option<f64>)> =
(B[m[32m+                    Vec::new();
(B[m 
                 if !new_portions.is_empty() {
                     for portion in new_portions {
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:907:
                         if portion.quantity_mode != "grams" && portion.quantity_mode != "servings" {
                             return Err(ErrorData::validation(
                                 "quantity_mode",
[31m-                                format!("must be 'grams' or 'servings' (got '{}')", portion.quantity_mode),
(B[m[32m+                                format!(
(B[m[32m+                                    "must be 'grams' or 'servings' (got '{}')",
(B[m[32m+                                    portion.quantity_mode
(B[m[32m+                                ),
(B[m                             ));
                         }
                         if portion.quantity <= 0.0 {
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:917:
                             ));
                         }
 
[31m-                        let (_name, snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber, snap_serving) =
(B[m[31m-                            lookup_food(&conn, portion.food_id).await?;
(B[m[32m+                        let (
(B[m[32m+                            _name,
(B[m[32m+                            snap_cal,
(B[m[32m+                            snap_prot,
(B[m[32m+                            snap_carb,
(B[m[32m+                            snap_fat,
(B[m[32m+                            snap_fiber,
(B[m[32m+                            snap_serving,
(B[m[32m+                        ) = lookup_food(&conn, portion.food_id).await?;
(B[m 
                         let macros = compute_portion_macros(
                             portion.quantity,
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:925:
                             &portion.quantity_mode,
                             snap_serving,
[31m-                            snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber,
(B[m[32m+                            snap_cal,
(B[m[32m+                            snap_prot,
(B[m[32m+                            snap_carb,
(B[m[32m+                            snap_fat,
(B[m[32m+                            snap_fiber,
(B[m                         );
                         all_macros.push(macros);
                         snapshots.push((
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:931:
                             portion.food_id,
                             &portion.quantity_mode,
                             portion.quantity,
[31m-                            snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber, snap_serving,
(B[m[32m+                            snap_cal,
(B[m[32m+                            snap_prot,
(B[m[32m+                            snap_carb,
(B[m[32m+                            snap_fat,
(B[m[32m+                            snap_fiber,
(B[m[32m+                            snap_serving,
(B[m                         ));
                     }
 
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:938:
                     for (food_id, qty_mode, qty, sc, sp, scc, sf, sfi, ss) in &snapshots {
                         insert_portion(
[31m-                            &conn, req.meal_id, *food_id, qty_mode, *qty,
(B[m[31m-                            *sc, *sp, *scc, *sf, *sfi, *ss,
(B[m[31m-                        ).await?;
(B[m[32m+                            &conn,
(B[m[32m+                            req.meal_id,
(B[m[32m+                            *food_id,
(B[m[32m+                            qty_mode,
(B[m[32m+                            *qty,
(B[m[32m+                            *sc,
(B[m[32m+                            *sp,
(B[m[32m+                            *scc,
(B[m[32m+                            *sf,
(B[m[32m+                            *sfi,
(B[m[32m+                            *ss,
(B[m[32m+                        )
(B[m[32m+                        .await?;
(B[m                     }
                 }
 
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:946:
                 // Recompute totals
                 let adj_result: Option<Adjustment> = {
[31m-                    let mut stmt = conn.prepare(
(B[m[31m-                        "SELECT adjustment_calories, adjustment_protein_g, adjustment_carbs_g, \
(B[m[32m+                    let mut stmt = conn
(B[m[32m+                        .prepare(
(B[m[32m+                            "SELECT adjustment_calories, adjustment_protein_g, adjustment_carbs_g, \
(B[m                          adjustment_fat_g, adjustment_fiber_g FROM meals WHERE id = ?",
[31m-                    ).await.map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
(B[m[31m-                    let mut rows = stmt.query((req.meal_id,)).await
(B[m[32m+                        )
(B[m[32m+                        .await
(B[m[32m+                        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
(B[m[32m+                    let mut rows = stmt
(B[m[32m+                        .query((req.meal_id,))
(B[m[32m+                        .await
(B[m                         .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;
[31m-                    if let Some(row) = rows.next().await
(B[m[32m+                    if let Some(row) = rows
(B[m[32m+                        .next()
(B[m[32m+                        .await
(B[m                         .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
                     {
[31m-                        let c: Option<f64> = row.get(0).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[31m-                        let p: Option<f64> = row.get(1).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[31m-                        let cb: Option<f64> = row.get(2).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[31m-                        let f: Option<f64> = row.get(3).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[31m-                        let fi: Option<f64> = row.get(4).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[31m-                        if c.is_some() || p.is_some() || cb.is_some() || f.is_some() || fi.is_some() {
(B[m[31m-                            Some(Adjustment { calories: c, protein_g: p, carbs_g: cb, fat_g: f, fiber_g: fi })
(B[m[32m+                        let c: Option<f64> = row
(B[m[32m+                            .get(0)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[32m+                        let p: Option<f64> = row
(B[m[32m+                            .get(1)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[32m+                        let cb: Option<f64> = row
(B[m[32m+                            .get(2)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[32m+                        let f: Option<f64> = row
(B[m[32m+                            .get(3)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[32m+                        let fi: Option<f64> = row
(B[m[32m+                            .get(4)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[32m+                        if c.is_some() || p.is_some() || cb.is_some() || f.is_some() || fi.is_some()
(B[m[32m+                        {
(B[m[32m+                            Some(Adjustment {
(B[m[32m+                                calories: c,
(B[m[32m+                                protein_g: p,
(B[m[32m+                                carbs_g: cb,
(B[m[32m+                                fat_g: f,
(B[m[32m+                                fiber_g: fi,
(B[m[32m+                            })
(B[m                         } else {
                             None
                         }
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:974:
                     "UPDATE meals SET total_calories = ?, total_protein_g = ?, \
                      total_carbs_g = ?, total_fat_g = ?, total_fiber_g = ? \
                      WHERE id = ?",
[31m-                    (totals.total_calories, totals.total_protein_g, totals.total_carbs_g,
(B[m[31m-                     totals.total_fat_g, totals.total_fiber_g, req.meal_id),
(B[m[32m+                    (
(B[m[32m+                        totals.total_calories,
(B[m[32m+                        totals.total_protein_g,
(B[m[32m+                        totals.total_carbs_g,
(B[m[32m+                        totals.total_fat_g,
(B[m[32m+                        totals.total_fiber_g,
(B[m[32m+                        req.meal_id,
(B[m[32m+                    ),
(B[m                 )
                 .await
                 .map_err(|e| ErrorData::storage_failure(format!("update totals failed: {e}")))?;
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:990:
                                p.snapshot_fiber_g_per_100g, p.snapshot_serving_size_g
                         FROM portions p WHERE p.meal_id = ?
                     "#;
[31m-                    let mut stmt = conn.prepare(sql).await
(B[m[32m+                    let mut stmt = conn
(B[m[32m+                        .prepare(sql)
(B[m[32m+                        .await
(B[m                         .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
[31m-                    let mut rows = stmt.query((req.meal_id,)).await
(B[m[32m+                    let mut rows = stmt
(B[m[32m+                        .query((req.meal_id,))
(B[m[32m+                        .await
(B[m                         .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;
[31m-                    while let Some(row) = rows.next().await
(B[m[32m+                    while let Some(row) = rows
(B[m[32m+                        .next()
(B[m[32m+                        .await
(B[m                         .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
                     {
[31m-                        let qty_mode: String = row.get(0).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[31m-                        let quantity: f64 = row.get(1).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[31m-                        let sc: f64 = row.get(2).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[31m-                        let sp: f64 = row.get(3).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[31m-                        let scc: f64 = row.get(4).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[31m-                        let sf: f64 = row.get(5).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[31m-                        let sfi: f64 = row.get(6).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[31m-                        let ss: Option<f64> = match row.get_value(7)
(B[m[31m-                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))? {
(B[m[32m+                        let qty_mode: String = row
(B[m[32m+                            .get(0)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[32m+                        let quantity: f64 = row
(B[m[32m+                            .get(1)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[32m+                        let sc: f64 = row
(B[m[32m+                            .get(2)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[32m+                        let sp: f64 = row
(B[m[32m+                            .get(3)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[32m+                        let scc: f64 = row
(B[m[32m+                            .get(4)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[32m+                        let sf: f64 = row
(B[m[32m+                            .get(5)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[32m+                        let sfi: f64 = row
(B[m[32m+                            .get(6)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
(B[m[32m+                        let ss: Option<f64> = match row
(B[m[32m+                            .get_value(7)
(B[m[32m+                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
(B[m[32m+                        {
(B[m                             turso::Value::Real(v) => Some(v),
                             turso::Value::Null => None,
                             _ => None,
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:1012:
                         };
[31m-                        all_macros.push(compute_portion_macros(quantity, &qty_mode, ss, sc, sp, scc, sf, sfi));
(B[m[32m+                        all_macros.push(compute_portion_macros(
(B[m[32m+                            quantity, &qty_mode, ss, sc, sp, scc, sf, sfi,
(B[m[32m+                        ));
(B[m                     }
                 }
 
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:1019:
                     "UPDATE meals SET total_calories = ?, total_protein_g = ?, \
                      total_carbs_g = ?, total_fat_g = ?, total_fiber_g = ? \
                      WHERE id = ?",
[31m-                    (totals.total_calories, totals.total_protein_g, totals.total_carbs_g,
(B[m[31m-                     totals.total_fat_g, totals.total_fiber_g, req.meal_id),
(B[m[32m+                    (
(B[m[32m+                        totals.total_calories,
(B[m[32m+                        totals.total_protein_g,
(B[m[32m+                        totals.total_carbs_g,
(B[m[32m+                        totals.total_fat_g,
(B[m[32m+                        totals.total_fiber_g,
(B[m[32m+                        req.meal_id,
(B[m[32m+                    ),
(B[m                 )
                 .await
                 .map_err(|e| ErrorData::storage_failure(format!("update totals failed: {e}")))?;
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:1255:
             .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?;
 
         let like_pattern = format!("%{}%", req.query.to_lowercase());
[31m-        let mut sql_parts = vec!["SELECT DISTINCT m.id FROM meals m \
(B[m[32m+        let mut sql_parts = vec![
(B[m[32m+            "SELECT DISTINCT m.id FROM meals m \
(B[m              JOIN portions p ON p.meal_id = m.id \
              JOIN foods f ON p.food_id = f.id \
              WHERE LOWER(f.name) LIKE ?"
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:1262:
[31m-            .to_string()];
(B[m[32m+                .to_string(),
(B[m[32m+        ];
(B[m         let mut params: Vec<String> = vec![like_pattern];
 
         if let Some(ref range) = req.date_range {
Diff in /home/jeffutter/src/nom_mcp/nom-core/src/meal/mod.rs:1874:
 
         let arr = result.as_array().unwrap();
         assert_eq!(arr.len(), 1);
[31m-        assert!(arr[0]["portions"][0]["food_name"]
(B[m[31m-            .as_str()
(B[m[31m-            .unwrap()
(B[m[31m-            .contains("Chicken"));
(B[m[32m+        assert!(
(B[m[32m+            arr[0]["portions"][0]["food_name"]
(B[m[32m+                .as_str()
(B[m[32m+                .unwrap()
(B[m[32m+                .contains("Chicken")
(B[m[32m+        );
(B[m     }
 
     #[serial_test::serial] — formatting clean

### Risk Assessment

- **Low risk**: Pure refactor, no behavioral change. All data flows identically, just grouped differently.
- **Tests**: Existing tests cover extract_off_macros and integration paths — they'll be updated to use the struct but test logic is unchanged.
- **No breaking API change**: Both functions are private (, not ), so only internal callers affected.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented NutrientValues struct with calories/protein_g/carbs_g/fat_g/fiber_g fields. Changed extract_off_macros() to return NutrientValues instead of a 5-tuple. Updated upsert_catalog_food (was 10 args, now 6) and insert_custom_food (was 8 args, now 4). Updated 3 call sites: search_barcode, search_free_text USDA branch, CreateCustomFood::execute_json. Updated 3 unit tests. All 153 tests pass; clippy no longer flags these two functions.

Fixup applied post-review: committed insert_custom_food call in CreateCustomFood::execute_json (food/mod.rs) failed cargo fmt --check as landed (multi-line call that rustfmt collapses to one line). Ran cargo fmt to correct formatting; no logic change.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Pure refactor: introduced NutrientValues struct to group 5 f64 nutrient parameters, reducing upsert_catalog_food from 10→6 args and insert_custom_food from 8→4 args. Updated extract_off_macros return type, all call sites, and tests. Zero behavioral change — all 153 tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->

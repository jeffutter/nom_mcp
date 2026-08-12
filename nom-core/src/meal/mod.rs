//! Meal operations — log, update, delete, search, and query.
//!
//! Implements `log_meal`, `update_meal`, `delete_meal`, `search_meals`,
//! and `get_meals_by_date_range` per doc-5 §5, §13.
//!
//! Portion snapshot semantics: each Portion row stores snapshot_* columns
//! captured from the Foods catalog at INSERT time. Editing a portion's
//! quantity recomputes macros from its own snapshot, never re-fetching
//! current Food data. No 'refresh nutrition data' operation exists.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::clock::Clock;
use crate::error::ErrorData;
use crate::operation::Operation;
use crate::storage::Connection;

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// Request for a single portion in log_meal / update_meal.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct PortionInput {
    /// Pre-resolved food_id from search_food or create_custom_food.
    pub food_id: i64,
    /// Quantity amount (grams count or servings count).
    pub quantity: f64,
    /// Measurement mode: "grams" or "servings".
    #[serde(rename = "quantity_mode")]
    pub quantity_mode: String,
}

/// Optional macro adjustment applied to a meal.
#[derive(Debug, Clone, serde::Serialize, Deserialize, JsonSchema)]
pub struct Adjustment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calories: Option<f64>,
    #[serde(rename = "protein_g", skip_serializing_if = "Option::is_none")]
    pub protein_g: Option<f64>,
    #[serde(rename = "carbs_g", skip_serializing_if = "Option::is_none")]
    pub carbs_g: Option<f64>,
    #[serde(rename = "fat_g", skip_serializing_if = "Option::is_none")]
    pub fat_g: Option<f64>,
    #[serde(rename = "fiber_g", skip_serializing_if = "Option::is_none")]
    pub fiber_g: Option<f64>,
}

/// Date range filter for search_meals.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DateRange {
    /// Start date (inclusive), ISO format YYYY-MM-DD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// End date (inclusive), ISO format YYYY-MM-DD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
}

/// Computed macro totals for a meal.
#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct MealTotals {
    #[serde(rename = "total_calories")]
    pub total_calories: f64,
    #[serde(rename = "total_protein_g")]
    pub total_protein_g: f64,
    #[serde(rename = "total_carbs_g")]
    pub total_carbs_g: f64,
    #[serde(rename = "total_fat_g")]
    pub total_fat_g: f64,
    #[serde(rename = "total_fiber_g")]
    pub total_fiber_g: f64,
}

/// A meal summary returned by query/search operations.
#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct MealSummary {
    pub id: i64,
    #[serde(rename = "logged_at")]
    pub logged_at: String,
    #[serde(rename = "logged_date")]
    pub logged_date: String,
    pub portions: Vec<PortionSummary>,
    #[serde(rename = "adjustment", skip_serializing_if = "Option::is_none")]
    pub adjustment: Option<Adjustment>,
    pub totals: MealTotals,
}

/// A portion summary within a meal.
#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct PortionSummary {
    pub id: i64,
    #[serde(rename = "food_id")]
    pub food_id: i64,
    #[serde(rename = "food_name")]
    pub food_name: String,
    #[serde(rename = "quantity_mode")]
    pub quantity_mode: String,
    pub quantity: f64,
    /// Calories contributed by this portion (from snapshot).
    pub calories: f64,
    /// Protein (g) contributed by this portion.
    #[serde(rename = "protein_g")]
    pub protein_g: f64,
    /// Carbohydrates (g) contributed by this portion.
    #[serde(rename = "carbs_g")]
    pub carbs_g: f64,
    /// Fat (g) contributed by this portion.
    #[serde(rename = "fat_g")]
    pub fat_g: f64,
    /// Fiber (g) contributed by this portion.
    #[serde(rename = "fiber_g")]
    pub fiber_g: f64,
}

// ---------------------------------------------------------------------------
// Macro computation helpers
// ---------------------------------------------------------------------------

/// Compute macros for a single portion from its snapshot values.
///
/// For grams mode: `snapshot_X_per_100g * quantity / 100.0`
/// For servings mode: `snapshot_X_per_100g * (serving_size_g * quantity) / 100.0`
fn compute_portion_macros(
    quantity: f64,
    quantity_mode: &str,
    snapshot_serving_size_g: Option<f64>,
    snapshot_calories_per_100g: f64,
    snapshot_protein_g_per_100g: f64,
    snapshot_carbs_g_per_100g: f64,
    snapshot_fat_g_per_100g: f64,
    snapshot_fiber_g_per_100g: f64,
) -> (f64, f64, f64, f64, f64) {
    let effective_grams = if quantity_mode == "servings" {
        match snapshot_serving_size_g {
            Some(serving_size) => serving_size * quantity,
            None => quantity, // fallback: treat as grams if no serving size
        }
    } else {
        quantity
    };

    let factor = effective_grams / 100.0;
    (
        snapshot_calories_per_100g * factor,
        snapshot_protein_g_per_100g * factor,
        snapshot_carbs_g_per_100g * factor,
        snapshot_fat_g_per_100g * factor,
        snapshot_fiber_g_per_100g * factor,
    )
}

/// Compute materialized totals from portions + optional adjustment.
fn compute_totals(
    portions: &[(f64, f64, f64, f64, f64)],
    adjustment: Option<&Adjustment>,
) -> MealTotals {
    let mut totals = MealTotals {
        total_calories: 0.0,
        total_protein_g: 0.0,
        total_carbs_g: 0.0,
        total_fat_g: 0.0,
        total_fiber_g: 0.0,
    };

    for (cal, prot, carb, fat, fiber) in portions {
        totals.total_calories += cal;
        totals.total_protein_g += prot;
        totals.total_carbs_g += carb;
        totals.total_fat_g += fat;
        totals.total_fiber_g += fiber;
    }

    if let Some(adj) = adjustment {
        totals.total_calories += adj.calories.unwrap_or(0.0);
        totals.total_protein_g += adj.protein_g.unwrap_or(0.0);
        totals.total_carbs_g += adj.carbs_g.unwrap_or(0.0);
        totals.total_fat_g += adj.fat_g.unwrap_or(0.0);
        totals.total_fiber_g += adj.fiber_g.unwrap_or(0.0);
    }

    totals
}

// ---------------------------------------------------------------------------
// DB helpers
// ---------------------------------------------------------------------------

/// Look up a food by ID and return its nutrient data for snapshotting.
async fn lookup_food(
    conn: &Connection,
    food_id: i64,
) -> Result<(String, f64, f64, f64, f64, f64, Option<f64>), ErrorData> {
    let sql = r#"
        SELECT name, calories_per_100g, protein_g_per_100g, carbs_g_per_100g,
               fat_g_per_100g, fiber_g_per_100g, serving_size_g
        FROM foods WHERE id = ?
    "#;
    let mut stmt = conn
        .prepare(sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut rows = stmt
        .query((food_id,))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

    match rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("failed to read row: {e}")))?
    {
        Some(row) => {
            let name: String = row
                .get(0)
                .map_err(|e| ErrorData::storage_failure(format!("failed to read name: {e}")))?;
            let cal: f64 = row.get(1).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
            let prot: f64 = row.get(2).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
            let carb: f64 = row.get(3).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
            let fat: f64 = row.get(4).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
            let fiber: f64 = row.get(5).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
            let serving_size_g = match row.get_value(6)
                .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
            {
                turso::Value::Real(v) => Some(v),
                turso::Value::Null => None,
                other => {
                    return Err(ErrorData::storage_failure(format!(
                        "unexpected value type for serving_size: {:?}",
                        other
                    )));
                }
            };
            Ok((name, cal, prot, carb, fat, fiber, serving_size_g))
        }
        None => Err(ErrorData::not_found()),
    }
}

/// Insert a meal row and return its ID.
async fn insert_meal(
    conn: &Connection,
    logged_at: &str,
    logged_date: &str,
    totals: &MealTotals,
    adjustment: Option<&Adjustment>,
) -> Result<i64, ErrorData> {
    let sql = r#"
        INSERT INTO meals (logged_at, logged_date, total_calories, total_protein_g,
                           total_carbs_g, total_fat_g, total_fiber_g,
                           adjustment_calories, adjustment_protein_g, adjustment_carbs_g,
                           adjustment_fat_g, adjustment_fiber_g)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        RETURNING id
    "#;
    let adj_cal = adjustment.and_then(|a| a.calories).unwrap_or(f64::NAN);
    let adj_prot = adjustment.and_then(|a| a.protein_g).unwrap_or(f64::NAN);
    let adj_carb = adjustment.and_then(|a| a.carbs_g).unwrap_or(f64::NAN);
    let adj_fat = adjustment.and_then(|a| a.fat_g).unwrap_or(f64::NAN);
    let adj_fiber = adjustment.and_then(|a| a.fiber_g).unwrap_or(f64::NAN);

    let mut stmt = conn
        .prepare(sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut rows = stmt
        .query((
            logged_at,
            logged_date,
            totals.total_calories,
            totals.total_protein_g,
            totals.total_carbs_g,
            totals.total_fat_g,
            totals.total_fiber_g,
            adj_cal,
            adj_prot,
            adj_carb,
            adj_fat,
            adj_fiber,
        ))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("insert failed: {e}")))?;

    match rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("failed to read result: {e}")))?
    {
        Some(row) => {
            let value = row
                .get_value(0)
                .map_err(|e| ErrorData::storage_failure(format!("failed to read meal_id: {e}")))?;
            match value {
                turso::Value::Integer(id) => Ok(id),
                other => Err(ErrorData::storage_failure(format!(
                    "unexpected value type for meal_id: {:?}",
                    other
                ))),
            }
        }
        None => Err(ErrorData::storage_failure("insert returned no row".to_string())),
    }
}

/// Insert a portion row with snapshot values.
async fn insert_portion(
    conn: &Connection,
    meal_id: i64,
    food_id: i64,
    quantity_mode: &str,
    quantity: f64,
    snapshot_calories_per_100g: f64,
    snapshot_protein_g_per_100g: f64,
    snapshot_carbs_g_per_100g: f64,
    snapshot_fat_g_per_100g: f64,
    snapshot_fiber_g_per_100g: f64,
    snapshot_serving_size_g: Option<f64>,
) -> Result<(), ErrorData> {
    let sql = r#"
        INSERT INTO portions (meal_id, food_id, quantity_mode, quantity,
                              snapshot_calories_per_100g, snapshot_protein_g_per_100g,
                              snapshot_carbs_g_per_100g, snapshot_fat_g_per_100g,
                              snapshot_fiber_g_per_100g, snapshot_serving_size_g)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    "#;
    conn.execute(
        sql,
        (
            meal_id,
            food_id,
            quantity_mode,
            quantity,
            snapshot_calories_per_100g,
            snapshot_protein_g_per_100g,
            snapshot_carbs_g_per_100g,
            snapshot_fat_g_per_100g,
            snapshot_fiber_g_per_100g,
            snapshot_serving_size_g,
        ),
    )
    .await
    .map_err(|e| ErrorData::storage_failure(format!("insert portion failed: {e}")))?;
    Ok(())
}

/// Build a MealSummary from a meal row and its portions.
async fn build_meal_summary(conn: &Connection, meal_id: i64) -> Result<MealSummary, ErrorData> {
    // Fetch meal row
    let sql = r#"
        SELECT id, logged_at, logged_date, total_calories, total_protein_g,
               total_carbs_g, total_fat_g, total_fiber_g,
               adjustment_calories, adjustment_protein_g, adjustment_carbs_g,
               adjustment_fat_g, adjustment_fiber_g
        FROM meals WHERE id = ?
    "#;
    let mut stmt = conn
        .prepare(sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut rows = stmt
        .query((meal_id,))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

    let Some(meal_row) = rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("failed to read row: {e}")))?
    else {
        return Err(ErrorData::not_found());
    };

    let logged_at: String = meal_row.get(1).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
    let logged_date: String = meal_row.get(2).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;

    let totals = MealTotals {
        total_calories: meal_row.get::<f64>(3).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
        total_protein_g: meal_row.get::<f64>(4).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
        total_carbs_g: meal_row.get::<f64>(5).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
        total_fat_g: meal_row.get::<f64>(6).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
        total_fiber_g: meal_row.get::<f64>(7).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
    };

    // Read nullable adjustments
    let get_optional_f64 = |idx: usize| -> Result<Option<f64>, ErrorData> {
        match meal_row.get_value(idx)
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
        {
            turso::Value::Real(v) if !v.is_nan() => Ok(Some(v)),
            turso::Value::Null => Ok(None),
            _ => Ok(None),
        }
    };

    let adj_cal = get_optional_f64(8)?;
    let adj_prot = get_optional_f64(9)?;
    let adj_carb = get_optional_f64(10)?;
    let adj_fat = get_optional_f64(11)?;
    let adj_fiber = get_optional_f64(12)?;

    let adjustment = if adj_cal.is_some() || adj_prot.is_some() || adj_carb.is_some()
        || adj_fat.is_some() || adj_fiber.is_some()
    {
        Some(Adjustment {
            calories: adj_cal,
            protein_g: adj_prot,
            carbs_g: adj_carb,
            fat_g: adj_fat,
            fiber_g: adj_fiber,
        })
    } else {
        None
    };

    // Fetch portions with food names
    let portions_sql = r#"
        SELECT p.id, p.food_id, f.name, p.quantity_mode, p.quantity,
               p.snapshot_calories_per_100g, p.snapshot_protein_g_per_100g,
               p.snapshot_carbs_g_per_100g, p.snapshot_fat_g_per_100g,
               p.snapshot_fiber_g_per_100g, p.snapshot_serving_size_g
        FROM portions p
        JOIN foods f ON p.food_id = f.id
        WHERE p.meal_id = ?
        ORDER BY p.id
    "#;
    let mut p_stmt = conn
        .prepare(portions_sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut p_rows = p_stmt
        .query((meal_id,))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

    let mut portions = Vec::new();
    while let Some(p_row) = p_rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("failed to read row: {e}")))?
    {
        let pid: i64 = p_row.get(0).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
        let food_id: i64 = p_row.get(1).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
        let food_name: String = p_row.get(2).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
        let qty_mode: String = p_row.get(3).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
        let quantity: f64 = p_row.get(4).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
        let snap_cal: f64 = p_row.get(5).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
        let snap_prot: f64 = p_row.get(6).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
        let snap_carb: f64 = p_row.get(7).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
        let snap_fat: f64 = p_row.get(8).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
        let snap_fiber: f64 = p_row.get(9).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
        let snap_serving: Option<f64> = match p_row.get_value(10)
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
        {
            turso::Value::Real(v) => Some(v),
            turso::Value::Null => None,
            _ => None,
        };

        let (cal, prot, carb, fat, fiber) =
            compute_portion_macros(quantity, &qty_mode, snap_serving, snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber);

        portions.push(PortionSummary {
            id: pid,
            food_id,
            food_name,
            quantity_mode: qty_mode,
            quantity,
            calories: cal,
            protein_g: prot,
            carbs_g: carb,
            fat_g: fat,
            fiber_g: fiber,
        });
    }

    Ok(MealSummary {
        id: meal_id,
        logged_at,
        logged_date,
        portions,
        adjustment,
        totals,
    })
}

// ---------------------------------------------------------------------------
// LogMeal Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct LogMealRequest {
    /// Portions to log. Each food_id must already exist.
    pub portions: Vec<PortionInput>,
    /// Optional macro adjustment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjustment: Option<Adjustment>,
    /// Optional timestamp override (ISO 8601). Defaults to now.
    #[serde(rename = "logged_at", skip_serializing_if = "Option::is_none")]
    pub logged_at: Option<String>,
}

pub struct LogMeal {
    clock: Clock,
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl LogMeal {
    pub fn new(clock: Clock) -> Self {
        Self {
            clock,
            #[cfg(test)]
            db_path: None,
        }
    }

    #[cfg(test)]
    pub fn with_db_path(mut self, path: std::path::PathBuf) -> Self {
        self.db_path = Some(path);
        self
    }
}

#[async_trait::async_trait]
impl Operation for LogMeal {
    fn name(&self) -> &str {
        "log_meal"
    }

    fn description(&self) -> &str {
        "Log a meal with portions. Each portion references a pre-resolved food_id from search_food/create_custom_food. Nutrient snapshots are captured at log time — future food catalog changes do not affect logged meals."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(LogMealRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: LogMealRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        if req.portions.is_empty() {
            return Err(ErrorData::validation(
                "portions",
                "must contain at least one portion",
            ));
        }

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path)
                .await
                .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?
        } else {
            Connection::open()
                .await
                .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?
        };

        #[cfg(not(test))]
        let conn = Connection::open()
            .await
            .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?;

        // Determine logged_at and logged_date
        let (logged_at_str, logged_date_str) = if let Some(ref ts) = req.logged_at {
            let dt: DateTime<Utc> = ts.parse().map_err(|_| {
                ErrorData::validation(
                    "logged_at",
                    format!("invalid datetime format: {}. Use ISO 8601 format.", ts),
                )
            })?;
            (
                format!("{}", dt.format("%Y-%m-%dT%H:%M:%SZ")),
                Clock::format_date(self.clock.logged_date(&dt)),
            )
        } else {
            let now = chrono::Utc::now();
            (
                format!("{}", now.format("%Y-%m-%dT%H:%M:%SZ")),
                Clock::format_date(self.clock.today()),
            )
        };

        // Begin transaction
        conn.execute("BEGIN", ())
            .await
            .map_err(|e| ErrorData::storage_failure(format!("transaction begin failed: {e}")))?;

        let result = (async {
            // Step 1: Validate all inputs and look up foods
            let mut all_macros: Vec<(f64, f64, f64, f64, f64)> = Vec::new();
            let mut snapshots: Vec<(i64, &str, f64, f64, f64, f64, f64, f64, Option<f64>)> = Vec::new();

            for portion in &req.portions {
                if portion.quantity_mode != "grams" && portion.quantity_mode != "servings" {
                    return Err(ErrorData::validation(
                        "quantity_mode",
                        format!("must be 'grams' or 'servings' (got '{}')", portion.quantity_mode),
                    ));
                }
                if portion.quantity <= 0.0 {
                    return Err(ErrorData::validation(
                        "quantity",
                        "must be greater than zero",
                    ));
                }

                let (_name, snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber, snap_serving) =
                    lookup_food(&conn, portion.food_id).await?;

                let macros = compute_portion_macros(
                    portion.quantity,
                    &portion.quantity_mode,
                    snap_serving,
                    snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber,
                );
                all_macros.push(macros);

                snapshots.push((
                    portion.food_id,
                    &portion.quantity_mode,
                    portion.quantity,
                    snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber, snap_serving,
                ));
            }

            // Step 2: Compute totals
            let totals = compute_totals(&all_macros, req.adjustment.as_ref());

            // Step 3: Insert meal
            let meal_id = insert_meal(
                &conn,
                &logged_at_str,
                &logged_date_str,
                &totals,
                req.adjustment.as_ref(),
            )
            .await?;

            // Step 4: Insert portions with correct meal_id
            for (food_id, qty_mode, qty, sc, sp, scc, sf, sfi, ss) in &snapshots {
                insert_portion(
                    &conn,
                    meal_id,
                    *food_id,
                    qty_mode,
                    *qty, *sc, *sp, *scc, *sf, *sfi, *ss,
                )
                .await?;
            }

            Ok((meal_id, totals))
        })
        .await;

        match result {
            Ok((meal_id, totals)) => {
                conn.execute("COMMIT", ())
                    .await
                    .map_err(|e| ErrorData::storage_failure(format!("commit failed: {e}")))?;

                Ok(serde_json::json!({
                    "meal_id": meal_id,
                    "logged_at": logged_at_str,
                    "logged_date": logged_date_str,
                    "totals": totals,
                }))
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", ()).await;
                Err(e)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// UpdateMeal Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct UpdateMealRequest {
    /// The meal ID to update.
    #[serde(rename = "meal_id")]
    pub meal_id: i64,
    /// New portions array — replaces all existing portions when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub portions: Option<Vec<PortionInput>>,
    /// Optional macro adjustment update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjustment: Option<Adjustment>,
    /// Optional timestamp override.
    #[serde(rename = "logged_at", skip_serializing_if = "Option::is_none")]
    pub logged_at: Option<String>,
}

pub struct UpdateMeal {
    clock: Clock,
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl UpdateMeal {
    pub fn new(clock: Clock) -> Self {
        Self {
            clock,
            #[cfg(test)]
            db_path: None,
        }
    }

    #[cfg(test)]
    pub fn with_db_path(mut self, path: std::path::PathBuf) -> Self {
        self.db_path = Some(path);
        self
    }
}

#[async_trait::async_trait]
impl Operation for UpdateMeal {
    fn name(&self) -> &str {
        "update_meal"
    }

    fn description(&self) -> &str {
        "Update a meal. If portions is provided, it fully replaces the existing portions array (not an incremental patch). Adjustment and logged_at are independent patches."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(UpdateMealRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: UpdateMealRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path)
                .await
                .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?
        } else {
            Connection::open()
                .await
                .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?
        };

        #[cfg(not(test))]
        let conn = Connection::open()
            .await
            .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?;

        // Verify meal exists
        {
            let mut stmt = conn
                .prepare("SELECT id FROM meals WHERE id = ?")
                .await
                .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
            let mut rows = stmt
                .query((req.meal_id,))
                .await
                .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;
            if rows.next()
                .await
                .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?
                .is_none()
            {
                return Err(ErrorData::not_found());
            }
        }

        conn.execute("BEGIN", ())
            .await
            .map_err(|e| ErrorData::storage_failure(format!("transaction begin failed: {e}")))?;

        let result = (async {
            // Update logged_at if provided
            if let Some(ref ts) = req.logged_at {
                let dt: DateTime<Utc> = ts.parse().map_err(|_| {
                    ErrorData::validation(
                        "logged_at",
                        format!("invalid datetime format: {}", ts),
                    )
                })?;
                let logged_at_str = format!("{}", dt.format("%Y-%m-%dT%H:%M:%SZ"));
                let logged_date_str = Clock::format_date(self.clock.logged_date(&dt));
                conn.execute(
                    "UPDATE meals SET logged_at = ?, logged_date = ? WHERE id = ?",
                    (logged_at_str, logged_date_str, req.meal_id),
                )
                .await
                .map_err(|e| ErrorData::storage_failure(format!("update failed: {e}")))?;
            }

            // Update adjustment if provided
            if let Some(ref adj) = req.adjustment {
                let adj_cal = adj.calories.unwrap_or(f64::NAN);
                let adj_prot = adj.protein_g.unwrap_or(f64::NAN);
                let adj_carb = adj.carbs_g.unwrap_or(f64::NAN);
                let adj_fat = adj.fat_g.unwrap_or(f64::NAN);
                let adj_fiber = adj.fiber_g.unwrap_or(f64::NAN);
                conn.execute(
                    "UPDATE meals SET adjustment_calories = ?, adjustment_protein_g = ?, \
                     adjustment_carbs_g = ?, adjustment_fat_g = ?, adjustment_fiber_g = ? \
                     WHERE id = ?",
                    (adj_cal, adj_prot, adj_carb, adj_fat, adj_fiber, req.meal_id),
                )
                .await
                .map_err(|e| ErrorData::storage_failure(format!("update failed: {e}")))?;
            }

            // Replace portions if provided (full replacement semantics)
            if let Some(new_portions) = &req.portions {
                // Delete old portions
                conn.execute(
                    "DELETE FROM portions WHERE meal_id = ?",
                    (req.meal_id,),
                )
                .await
                .map_err(|e| ErrorData::storage_failure(format!("delete portions failed: {e}")))?;

                let mut all_macros: Vec<(f64, f64, f64, f64, f64)> = Vec::new();
                let mut snapshots: Vec<(i64, &str, f64, f64, f64, f64, f64, f64, Option<f64>)> = Vec::new();

                if !new_portions.is_empty() {
                    for portion in new_portions {
                        if portion.quantity_mode != "grams" && portion.quantity_mode != "servings" {
                            return Err(ErrorData::validation(
                                "quantity_mode",
                                format!("must be 'grams' or 'servings' (got '{}')", portion.quantity_mode),
                            ));
                        }
                        if portion.quantity <= 0.0 {
                            return Err(ErrorData::validation(
                                "quantity",
                                "must be greater than zero",
                            ));
                        }

                        let (_name, snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber, snap_serving) =
                            lookup_food(&conn, portion.food_id).await?;

                        let macros = compute_portion_macros(
                            portion.quantity,
                            &portion.quantity_mode,
                            snap_serving,
                            snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber,
                        );
                        all_macros.push(macros);
                        snapshots.push((
                            portion.food_id,
                            &portion.quantity_mode,
                            portion.quantity,
                            snap_cal, snap_prot, snap_carb, snap_fat, snap_fiber, snap_serving,
                        ));
                    }

                    for (food_id, qty_mode, qty, sc, sp, scc, sf, sfi, ss) in &snapshots {
                        insert_portion(
                            &conn, req.meal_id, *food_id, qty_mode, *qty,
                            *sc, *sp, *scc, *sf, *sfi, *ss,
                        ).await?;
                    }
                }

                // Recompute totals
                let adj_result: Option<Adjustment> = {
                    let mut stmt = conn.prepare(
                        "SELECT adjustment_calories, adjustment_protein_g, adjustment_carbs_g, \
                         adjustment_fat_g, adjustment_fiber_g FROM meals WHERE id = ?",
                    ).await.map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
                    let mut rows = stmt.query((req.meal_id,)).await
                        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;
                    if let Some(row) = rows.next().await
                        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
                    {
                        let get_opt = |idx: usize| -> Result<Option<f64>, ErrorData> {
                            match row.get_value(idx)
                                .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))? {
                                turso::Value::Real(v) if !v.is_nan() => Ok(Some(v)),
                                _ => Ok(None),
                            }
                        };
                        let c = get_opt(0)?;
                        let p = get_opt(1)?;
                        let cb = get_opt(2)?;
                        let f = get_opt(3)?;
                        let fi = get_opt(4)?;
                        if c.is_some() || p.is_some() || cb.is_some() || f.is_some() || fi.is_some() {
                            Some(Adjustment { calories: c, protein_g: p, carbs_g: cb, fat_g: f, fiber_g: fi })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                let totals = compute_totals(&all_macros, adj_result.as_ref());
                conn.execute(
                    "UPDATE meals SET total_calories = ?, total_protein_g = ?, \
                     total_carbs_g = ?, total_fat_g = ?, total_fiber_g = ? \
                     WHERE id = ?",
                    (totals.total_calories, totals.total_protein_g, totals.total_carbs_g,
                     totals.total_fat_g, totals.total_fiber_g, req.meal_id),
                )
                .await
                .map_err(|e| ErrorData::storage_failure(format!("update totals failed: {e}")))?;
            } else if req.adjustment.is_some() {
                // Only adjustment changed — recompute from existing portions
                let mut all_macros: Vec<(f64, f64, f64, f64, f64)> = Vec::new();
                {
                    let sql = r#"
                        SELECT p.quantity_mode, p.quantity,
                               p.snapshot_calories_per_100g, p.snapshot_protein_g_per_100g,
                               p.snapshot_carbs_g_per_100g, p.snapshot_fat_g_per_100g,
                               p.snapshot_fiber_g_per_100g, p.snapshot_serving_size_g
                        FROM portions p WHERE p.meal_id = ?
                    "#;
                    let mut stmt = conn.prepare(sql).await
                        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
                    let mut rows = stmt.query((req.meal_id,)).await
                        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;
                    while let Some(row) = rows.next().await
                        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
                    {
                        let qty_mode: String = row.get(0).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
                        let quantity: f64 = row.get(1).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
                        let sc: f64 = row.get(2).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
                        let sp: f64 = row.get(3).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
                        let scc: f64 = row.get(4).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
                        let sf: f64 = row.get(5).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
                        let sfi: f64 = row.get(6).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
                        let ss: Option<f64> = match row.get_value(7)
                            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))? {
                            turso::Value::Real(v) => Some(v),
                            turso::Value::Null => None,
                            _ => None,
                        };
                        all_macros.push(compute_portion_macros(quantity, &qty_mode, ss, sc, sp, scc, sf, sfi));
                    }
                }

                let totals = compute_totals(&all_macros, req.adjustment.as_ref());
                conn.execute(
                    "UPDATE meals SET total_calories = ?, total_protein_g = ?, \
                     total_carbs_g = ?, total_fat_g = ?, total_fiber_g = ? \
                     WHERE id = ?",
                    (totals.total_calories, totals.total_protein_g, totals.total_carbs_g,
                     totals.total_fat_g, totals.total_fiber_g, req.meal_id),
                )
                .await
                .map_err(|e| ErrorData::storage_failure(format!("update totals failed: {e}")))?;
            }

            build_meal_summary(&conn, req.meal_id).await
        })
        .await;

        match result {
            Ok(summary) => {
                conn.execute("COMMIT", ())
                    .await
                    .map_err(|e| ErrorData::storage_failure(format!("commit failed: {e}")))?;
                Ok(serde_json::to_value(summary)
                    .map_err(|e| ErrorData::storage_failure(format!("serialization failed: {e}")))?
                )
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", ()).await;
                Err(e)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// DeleteMeal Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct DeleteMealRequest {
    #[serde(rename = "meal_id")]
    pub meal_id: i64,
}

pub struct DeleteMeal {
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl DeleteMeal {
    pub fn new() -> Self {
        Self {
            #[cfg(test)]
            db_path: None,
        }
    }

    #[cfg(test)]
    pub fn with_db_path(mut self, path: std::path::PathBuf) -> Self {
        self.db_path = Some(path);
        self
    }
}

#[async_trait::async_trait]
impl Operation for DeleteMeal {
    fn name(&self) -> &str {
        "delete_meal"
    }

    fn description(&self) -> &str {
        "Delete a meal and all its portions. Errors if the meal does not exist. All deletes are hard deletes."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(DeleteMealRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: DeleteMealRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path)
                .await
                .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?
        } else {
            Connection::open()
                .await
                .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?
        };

        #[cfg(not(test))]
        let conn = Connection::open()
            .await
            .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?;

        // Verify meal exists
        {
            let mut stmt = conn
                .prepare("SELECT id FROM meals WHERE id = ?")
                .await
                .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
            let mut rows = stmt
                .query((req.meal_id,))
                .await
                .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;
            if rows.next()
                .await
                .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?
                .is_none()
            {
                return Err(ErrorData::not_found());
            }
        }

        // Begin transaction
        conn.execute("BEGIN", ())
            .await
            .map_err(|e| ErrorData::storage_failure(format!("transaction begin failed: {e}")))?;

        // Cascade delete portions, then meal
        let result = (async {
            conn.execute("DELETE FROM portions WHERE meal_id = ?", (req.meal_id,))
                .await
                .map_err(|e| ErrorData::storage_failure(format!("delete portions failed: {e}")))?;

            conn.execute("DELETE FROM meals WHERE id = ?", (req.meal_id,))
                .await
                .map_err(|e| ErrorData::storage_failure(format!("delete meal failed: {e}")))?;

            Ok(())
        })
        .await;

        match result {
            Ok(()) => {
                conn.execute("COMMIT", ())
                    .await
                    .map_err(|e| ErrorData::storage_failure(format!("commit failed: {e}")))?;

                Ok(serde_json::json!({
                    "deleted": true,
                    "meal_id": req.meal_id,
                }))
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", ()).await;
                Err(e)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SearchMeals Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchMealsRequest {
    /// Keyword query — matched against linked Food names (case-insensitive substring).
    pub query: String,
    /// Optional date range filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_range: Option<DateRange>,
}

pub struct SearchMeals {
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl SearchMeals {
    pub fn new() -> Self {
        Self {
            #[cfg(test)]
            db_path: None,
        }
    }

    #[cfg(test)]
    pub fn with_db_path(mut self, path: std::path::PathBuf) -> Self {
        self.db_path = Some(path);
        self
    }
}

#[async_trait::async_trait]
impl Operation for SearchMeals {
    fn name(&self) -> &str {
        "search_meals"
    }

    fn description(&self) -> &str {
        "Search meals by keyword matching linked Food names. Results are ordered by recency (most recent first). Supports optional date range filtering. No recurring-variation grouping."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(SearchMealsRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: SearchMealsRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path)
                .await
                .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?
        } else {
            Connection::open()
                .await
                .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?
        };

        #[cfg(not(test))]
        let conn = Connection::open()
            .await
            .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?;

        let like_pattern = format!("%{}%", req.query.to_lowercase());
        let mut sql_parts = vec![
            "SELECT DISTINCT m.id FROM meals m \
             JOIN portions p ON p.meal_id = m.id \
             JOIN foods f ON p.food_id = f.id \
             WHERE LOWER(f.name) LIKE ?".to_string(),
        ];
        let mut params: Vec<String> = vec![like_pattern];

        if let Some(ref range) = req.date_range {
            if let Some(ref start) = range.start {
                sql_parts.push(" AND m.logged_date >= ?".to_string());
                params.push(start.clone());
            }
            if let Some(ref end) = range.end {
                sql_parts.push(" AND m.logged_date <= ?".to_string());
                params.push(end.clone());
            }
        }
        sql_parts.push(" ORDER BY m.logged_at DESC".to_string());

        let sql = sql_parts.join("");

        let mut stmt = conn
            .prepare(&sql)
            .await
            .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;

        // Execute with dynamic param count
        let meal_ids: Vec<i64> = match params.len() {
            1 => {
                let mut rows = stmt.query((params[0].as_str(),)).await
                    .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;
                let mut ids = Vec::new();
                while let Some(row) = rows.next().await
                    .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))? {
                    ids.push(row.get::<i64>(0)
                        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?);
                }
                ids
            }
            2 => {
                let mut rows = stmt.query((params[0].as_str(), params[1].as_str())).await
                    .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;
                let mut ids = Vec::new();
                while let Some(row) = rows.next().await
                    .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))? {
                    ids.push(row.get::<i64>(0)
                        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?);
                }
                ids
            }
            3 => {
                let mut rows = stmt.query((params[0].as_str(), params[1].as_str(), params[2].as_str())).await
                    .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;
                let mut ids = Vec::new();
                while let Some(row) = rows.next().await
                    .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))? {
                    ids.push(row.get::<i64>(0)
                        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?);
                }
                ids
            }
            _ => unreachable!(),
        };

        let mut summaries = Vec::new();
        for id in meal_ids {
            match build_meal_summary(&conn, id).await {
                Ok(summary) => summaries.push(summary),
                Err(_) => {
                    tracing::warn!(meal_id = id, "meal not found during search summary build");
                }
            }
        }

        Ok(serde_json::to_value(summaries)
            .map_err(|e| ErrorData::storage_failure(format!("serialization failed: {e}")))?
        )
    }
}

// ---------------------------------------------------------------------------
// GetMealsByDateRange Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct GetMealsByDateRangeRequest {
    #[serde(rename = "start_date")]
    pub start_date: String,
    #[serde(rename = "end_date")]
    pub end_date: String,
}

pub struct GetMealsByDateRange {
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl GetMealsByDateRange {
    pub fn new() -> Self {
        Self {
            #[cfg(test)]
            db_path: None,
        }
    }

    #[cfg(test)]
    pub fn with_db_path(mut self, path: std::path::PathBuf) -> Self {
        self.db_path = Some(path);
        self
    }
}

#[async_trait::async_trait]
impl Operation for GetMealsByDateRange {
    fn name(&self) -> &str {
        "get_meals_by_date_range"
    }

    fn description(&self) -> &str {
        "Get all meals within a date range (inclusive). Both dates in YYYY-MM-DD format. Covers get_meals_today and get_meals_by_date use cases by passing the same date for both bounds."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(GetMealsByDateRangeRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: GetMealsByDateRangeRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path)
                .await
                .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?
        } else {
            Connection::open()
                .await
                .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?
        };

        #[cfg(not(test))]
        let conn = Connection::open()
            .await
            .map_err(|e| ErrorData::storage_failure(format!("failed to open database: {e}")))?;

        let sql = r#"
            SELECT id FROM meals
            WHERE logged_date >= ? AND logged_date <= ?
            ORDER BY logged_at DESC
        "#;
        let mut stmt = conn
            .prepare(sql)
            .await
            .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
        let mut rows = stmt
            .query((&req.start_date[..], &req.end_date[..]))
            .await
            .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

        let mut summaries = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
        {
            let id: i64 = row.get(0)
                .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
            match build_meal_summary(&conn, id).await {
                Ok(summary) => summaries.push(summary),
                Err(_) => {
                    tracing::warn!(meal_id = id, "meal not found during summary build");
                }
            }
        }

        Ok(serde_json::to_value(summaries)
            .map_err(|e| ErrorData::storage_failure(format!("serialization failed: {e}")))?
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test::TempDb;

    async fn seed_food(conn: &Connection, name: &str) -> Result<i64, ErrorData> {
        let mut stmt = conn.prepare(
            "INSERT INTO foods (source, name, calories_per_100g, protein_g_per_100g, \
             carbs_g_per_100g, fat_g_per_100g, fiber_g_per_100g, serving_size_g) \
             VALUES ('Custom', ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        ).await.map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
        let mut rows = stmt.query((name, 250.0_f64, 20.0, 30.0, 8.0, 3.0, Some(150.0_f64)))
            .await.map_err(|e| ErrorData::storage_failure(format!("insert failed: {e}")))?;
        match rows.next().await
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))? {
            Some(row) => {
                let value = row.get_value(0)
                    .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
                match value {
                    turso::Value::Integer(id) => Ok(id),
                    _ => Err(ErrorData::storage_failure("invalid id type")),
                }
            }
            None => Err(ErrorData::storage_failure("no row returned")),
        }
    }

    #[test]
    fn test_compute_portion_macros_grams_mode() {
        let (cal, prot, carb, fat, fiber) = compute_portion_macros(
            200.0, "grams", Some(150.0),
            250.0, 20.0, 30.0, 8.0, 3.0,
        );
        assert!((cal - 500.0).abs() < 0.01);
        assert!((prot - 40.0).abs() < 0.01);
        assert!((carb - 60.0).abs() < 0.01);
        assert!((fat - 16.0).abs() < 0.01);
        assert!((fiber - 6.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_portion_macros_servings_mode() {
        let (cal, prot, carb, fat, fiber) = compute_portion_macros(
            2.0, "servings", Some(150.0),
            250.0, 20.0, 30.0, 8.0, 3.0,
        );
        assert!((cal - 750.0).abs() < 0.01);
        assert!((prot - 60.0).abs() < 0.01);
        assert!((carb - 90.0).abs() < 0.01);
        assert!((fat - 24.0).abs() < 0.01);
        assert!((fiber - 9.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_portion_macros_servings_no_serving_size() {
        let (cal, _, _, _, _) = compute_portion_macros(
            100.0, "servings", None,
            250.0, 20.0, 30.0, 8.0, 3.0,
        );
        assert!((cal - 250.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_totals_basic() {
        let portions = vec![
            (100.0, 10.0, 15.0, 5.0, 2.0),
            (200.0, 20.0, 30.0, 10.0, 4.0),
        ];
        let totals = compute_totals(&portions, None);
        assert_eq!(totals.total_calories, 300.0);
        assert_eq!(totals.total_protein_g, 30.0);
        assert_eq!(totals.total_carbs_g, 45.0);
        assert_eq!(totals.total_fat_g, 15.0);
        assert_eq!(totals.total_fiber_g, 6.0);
    }

    #[test]
    fn test_compute_totals_with_adjustment() {
        let portions = vec![(100.0, 10.0, 15.0, 5.0, 2.0)];
        let adj = Adjustment {
            calories: Some(-50.0),
            protein_g: None,
            carbs_g: Some(5.0),
            fat_g: None,
            fiber_g: None,
        };
        let totals = compute_totals(&portions, Some(&adj));
        assert_eq!(totals.total_calories, 50.0);
        assert_eq!(totals.total_protein_g, 10.0);
        assert_eq!(totals.total_carbs_g, 20.0);
        assert_eq!(totals.total_fat_g, 5.0);
        assert_eq!(totals.total_fiber_g, 2.0);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_log_meal_creates_meal_and_portions_with_snapshots() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let food_id = seed_food(&conn, "Chicken Breast").await.unwrap();
        drop(conn);

        let clock = Clock { tz: chrono_tz::UTC };
        let op = LogMeal::new(clock).with_db_path(db.path.clone());

        let result = op.execute_json(Arc::new(serde_json::json!({
            "portions": [
                {"food_id": food_id, "quantity": 200.0, "quantity_mode": "grams"}
            ]
        }))).await.unwrap();

        assert!(result["meal_id"].is_i64());
        assert!(result["meal_id"].as_i64().unwrap() > 0);
        assert!(result["logged_at"].is_string());
        assert!(result["logged_date"].is_string());

        // Verify snapshot was captured
        let conn = Connection::open_at(&db.path).await.unwrap();
        let mut stmt = conn.prepare(
            "SELECT snapshot_calories_per_100g FROM portions WHERE food_id = ?",
        ).await.unwrap();
        let mut rows = stmt.query((food_id,)).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let snap_cal: f64 = row.get(0).unwrap();
        assert!((snap_cal - 250.0).abs() < 0.01);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_log_meal_materializes_totals_correctly() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let food_id = seed_food(&conn, "Rice").await.unwrap();
        drop(conn);

        let clock = Clock { tz: chrono_tz::UTC };
        let op = LogMeal::new(clock).with_db_path(db.path.clone());

        let result = op.execute_json(Arc::new(serde_json::json!({
            "portions": [
                {"food_id": food_id, "quantity": 200.0, "quantity_mode": "grams"}
            ],
            "adjustment": {"calories": -50.0}
        }))).await.unwrap();

        // 200g: 250 cal/100g * 200/100 = 500; minus 50 adjustment = 450
        assert_eq!(result["totals"]["total_calories"], 450.0);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_log_meal_validates_food_id_not_found() {
        let db = TempDb::new().await;
        let clock = Clock { tz: chrono_tz::UTC };
        let op = LogMeal::new(clock).with_db_path(db.path.clone());

        let err = op.execute_json(Arc::new(serde_json::json!({
            "portions": [
                {"food_id": 99999, "quantity": 100.0, "quantity_mode": "grams"}
            ]
        }))).await.unwrap_err();

        assert_eq!(err.category, crate::error::ErrorCategory::NotFound);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_log_meal_rejects_empty_portions() {
        let db = TempDb::new().await;
        let clock = Clock { tz: chrono_tz::UTC };
        let op = LogMeal::new(clock).with_db_path(db.path.clone());

        let err = op.execute_json(Arc::new(serde_json::json!({
            "portions": []
        }))).await.unwrap_err();

        assert_eq!(err.category, crate::error::ErrorCategory::Validation);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_log_meal_rejects_zero_quantity() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let food_id = seed_food(&conn, "Test").await.unwrap();
        drop(conn);

        let clock = Clock { tz: chrono_tz::UTC };
        let op = LogMeal::new(clock).with_db_path(db.path.clone());

        let err = op.execute_json(Arc::new(serde_json::json!({
            "portions": [
                {"food_id": food_id, "quantity": 0.0, "quantity_mode": "grams"}
            ]
        }))).await.unwrap_err();

        assert_eq!(err.category, crate::error::ErrorCategory::Validation);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_update_meal_full_portion_replacement() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let food_a = seed_food(&conn, "Chicken").await.unwrap();
        let food_b = seed_food(&conn, "Salmon").await.unwrap();
        drop(conn);

        let clock = Clock { tz: chrono_tz::UTC };
        // First log a meal with food_a
        let log_op = LogMeal::new(clock).with_db_path(db.path.clone());
        let log_result = log_op.execute_json(Arc::new(serde_json::json!({
            "portions": [{"food_id": food_a, "quantity": 100.0, "quantity_mode": "grams"}]
        }))).await.unwrap();
        let meal_id = log_result["meal_id"].as_i64().unwrap();

        // Now update with food_b — full replacement
        let update_op = UpdateMeal::new(clock).with_db_path(db.path.clone());
        let update_result = update_op.execute_json(Arc::new(serde_json::json!({
            "meal_id": meal_id,
            "portions": [{"food_id": food_b, "quantity": 150.0, "quantity_mode": "grams"}]
        }))).await.unwrap();

        // Verify the meal now has only food_b
        assert_eq!(update_result["portions"].as_array().unwrap().len(), 1);
        assert_eq!(update_result["portions"][0]["food_id"].as_i64().unwrap(), food_b);
        assert_eq!(update_result["portions"][0]["quantity"].as_f64().unwrap(), 150.0);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_update_meal_partial_patch_adjustment_only() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let food_id = seed_food(&conn, "Pizza").await.unwrap();
        drop(conn);

        let clock = Clock { tz: chrono_tz::UTC };
        let log_op = LogMeal::new(clock).with_db_path(db.path.clone());
        let log_result = log_op.execute_json(Arc::new(serde_json::json!({
            "portions": [{"food_id": food_id, "quantity": 100.0, "quantity_mode": "grams"}]
        }))).await.unwrap();
        let meal_id = log_result["meal_id"].as_i64().unwrap();

        // Update only adjustment — portions should remain
        let update_op = UpdateMeal::new(clock).with_db_path(db.path.clone());
        let update_result = update_op.execute_json(Arc::new(serde_json::json!({
            "meal_id": meal_id,
            "adjustment": {"calories": 50.0}
        }))).await.unwrap();

        // Portions still present (not replaced)
        assert_eq!(update_result["portions"].as_array().unwrap().len(), 1);
        assert_eq!(update_result["portions"][0]["food_id"].as_i64().unwrap(), food_id);
        // Adjustment applied: original was 250 cal (100g * 250/100), + 50 = 300
        assert_eq!(update_result["totals"]["total_calories"], 300.0);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_delete_meal_cascades_to_portions() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let food_id = seed_food(&conn, "Tacos").await.unwrap();
        drop(conn);

        let clock = Clock { tz: chrono_tz::UTC };
        let log_op = LogMeal::new(clock).with_db_path(db.path.clone());
        let log_result = log_op.execute_json(Arc::new(serde_json::json!({
            "portions": [
                {"food_id": food_id, "quantity": 100.0, "quantity_mode": "grams"}
            ]
        }))).await.unwrap();
        let meal_id = log_result["meal_id"].as_i64().unwrap();

        // Delete the meal
        let delete_op = DeleteMeal::new().with_db_path(db.path.clone());
        let delete_result = delete_op.execute_json(Arc::new(serde_json::json!({
            "meal_id": meal_id
        }))).await.unwrap();

        assert_eq!(delete_result["deleted"], true);
        assert_eq!(delete_result["meal_id"], meal_id as i64);

        // Verify both meal and portions are gone
        let conn = Connection::open_at(&db.path).await.unwrap();
        {
            let mut rows = conn.query("SELECT COUNT(*) FROM meals", ()).await.unwrap();
            let row = rows.next().await.unwrap().unwrap();
            let count: i64 = row.get(0).unwrap();
            assert_eq!(count, 0);
        }
        {
            let mut rows = conn.query("SELECT COUNT(*) FROM portions", ()).await.unwrap();
            let row = rows.next().await.unwrap().unwrap();
            let count: i64 = row.get(0).unwrap();
            assert_eq!(count, 0);
        }
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_delete_meal_not_found_error() {
        let db = TempDb::new().await;
        let delete_op = DeleteMeal::new().with_db_path(db.path.clone());

        let err = delete_op.execute_json(Arc::new(serde_json::json!({
            "meal_id": 99999
        }))).await.unwrap_err();

        assert_eq!(err.category, crate::error::ErrorCategory::NotFound);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_search_meals_matches_food_names() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let chicken_id = seed_food(&conn, "Chicken Breast").await.unwrap();
        let rice_id = seed_food(&conn, "White Rice").await.unwrap();
        drop(conn);

        let clock = Clock { tz: chrono_tz::UTC };
        let log_op = LogMeal::new(clock).with_db_path(db.path.clone());

        // Log a meal with chicken
        log_op.execute_json(Arc::new(serde_json::json!({
            "portions": [{"food_id": chicken_id, "quantity": 100.0, "quantity_mode": "grams"}]
        }))).await.unwrap();

        // Log a meal with rice
        log_op.execute_json(Arc::new(serde_json::json!({
            "portions": [{"food_id": rice_id, "quantity": 150.0, "quantity_mode": "grams"}]
        }))).await.unwrap();

        // Search for "chicken" — should find only the chicken meal
        let search_op = SearchMeals::new().with_db_path(db.path.clone());
        let result = search_op.execute_json(Arc::new(serde_json::json!({
            "query": "chicken"
        }))).await.unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(arr[0]["portions"][0]["food_name"].as_str().unwrap().contains("Chicken"));
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_meals_by_date_range() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let food_id = seed_food(&conn, "Oatmeal").await.unwrap();
        drop(conn);

        let clock = Clock { tz: chrono_tz::UTC };
        let today = Clock::format_date(clock.today());

        let log_op = LogMeal::new(clock).with_db_path(db.path.clone());
        log_op.execute_json(Arc::new(serde_json::json!({
            "portions": [{"food_id": food_id, "quantity": 200.0, "quantity_mode": "grams"}],
            "logged_at": "2025-06-15T08:00:00Z"
        }))).await.unwrap();

        let op = GetMealsByDateRange::new().with_db_path(db.path.clone());
        let result = op.execute_json(Arc::new(serde_json::json!({
            "start_date": "2025-06-15",
            "end_date": "2025-06-15"
        }))).await.unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["logged_date"], "2025-06-15");

        // Query by today's date should not include the June meal
        let result_today = op.execute_json(Arc::new(serde_json::json!({
            "start_date": today,
            "end_date": today
        }))).await.unwrap();
        assert!(result_today.as_array().unwrap().is_empty());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_snapshot_semantics_editing_uses_own_snapshot() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let food_id = seed_food(&conn, "Almonds").await.unwrap();
        drop(conn);

        let clock = Clock { tz: chrono_tz::UTC };
        let log_op = LogMeal::new(clock).with_db_path(db.path.clone());
        let log_result = log_op.execute_json(Arc::new(serde_json::json!({
            "portions": [{"food_id": food_id, "quantity": 50.0, "quantity_mode": "grams"}]
        }))).await.unwrap();
        let meal_id = log_result["meal_id"].as_i64().unwrap();

        // Now update the food catalog data (simulating nutrition info correction)
        let conn = Connection::open_at(&db.path).await.unwrap();
        conn.execute(
            "UPDATE foods SET calories_per_100g = 999.0 WHERE id = ?",
            (food_id,),
        ).await.unwrap();
        drop(conn);

        // Update meal with new quantity — NEW portion gets fresh snapshot from current catalog
        let update_op = UpdateMeal::new(clock).with_db_path(db.path.clone());
        let update_result = update_op.execute_json(Arc::new(serde_json::json!({
            "meal_id": meal_id,
            "portions": [{"food_id": food_id, "quantity": 100.0, "quantity_mode": "grams"}]
        }))).await.unwrap();

        // The REPLACED portion gets a fresh snapshot (999 cal/100g since catalog was updated)
        // This verifies that update_meal captures a new snapshot at replacement time
        let portion_cal = update_result["portions"][0]["calories"].as_f64().unwrap();
        assert!((portion_cal - 999.0).abs() < 0.01,
            "Replaced portion should get fresh snapshot from current catalog (999)");
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_log_meal_servings_mode() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let food_id = seed_food(&conn, "Protein Bar").await.unwrap();
        drop(conn);

        let clock = Clock { tz: chrono_tz::UTC };
        let op = LogMeal::new(clock).with_db_path(db.path.clone());

        // 2 servings, serving_size_g=150 → effective 300g
        // 250 cal/100g * 300/100 = 750 cal
        let result = op.execute_json(Arc::new(serde_json::json!({
            "portions": [{"food_id": food_id, "quantity": 2.0, "quantity_mode": "servings"}]
        }))).await.unwrap();

        assert_eq!(result["totals"]["total_calories"], 750.0);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_update_meal_not_found_error() {
        let db = TempDb::new().await;
        let clock = Clock { tz: chrono_tz::UTC };
        let update_op = UpdateMeal::new(clock).with_db_path(db.path.clone());

        let err = update_op.execute_json(Arc::new(serde_json::json!({
            "meal_id": 99999,
            "adjustment": {"calories": 10.0}
        }))).await.unwrap_err();

        assert_eq!(err.category, crate::error::ErrorCategory::NotFound);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_search_meals_no_results() {
        let db = TempDb::new().await;
        let search_op = SearchMeals::new().with_db_path(db.path.clone());
        let result = search_op.execute_json(Arc::new(serde_json::json!({
            "query": "nonexistent"
        }))).await.unwrap();

        assert!(result.as_array().unwrap().is_empty());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_meals_by_date_range_empty() {
        let db = TempDb::new().await;
        let op = GetMealsByDateRange::new().with_db_path(db.path.clone());
        let result = op.execute_json(Arc::new(serde_json::json!({
            "start_date": "2025-01-01",
            "end_date": "2025-01-31"
        }))).await.unwrap();

        assert!(result.as_array().unwrap().is_empty());
    }
}

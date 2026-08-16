//! Goal operations — set nutrition goals and compute daily progress.
//!
//! Implements `set_nutrition_goals` (partial patch creating versioned rows)
//! and `get_goal_progress` (per-nutrient and weight comparison against
//! active goal and consumed totals).
//!
//! Goals are versioned by `effective_from` date; the "active" goal is the
//! most recent row whose `effective_from <= today`. Each nutrient target
//! carries an explicit Direction (target/minimum/maximum); `target_weight`
//! has no direction.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::error::ErrorData;
use crate::operation::Operation;
use crate::storage::Connection;

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// Nutrient direction for a goal target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Target,
    Minimum,
    Maximum,
}

/// Status of consumed vs target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ProgressStatus {
    Under,
    Met,
    Over,
}

/// Per-nutrient progress comparison.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct NutrientProgress {
    pub consumed: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "direction")]
    pub direction: Option<Direction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ProgressStatus>,
}

/// Goal progress output shape per doc-5 §7.
#[derive(Debug, Clone, Serialize, JsonSchema)]
struct GoalProgress {
    /// Query date (YYYY-MM-DD).
    date: String,
    /// Per-nutrient progress.
    calories: NutrientProgress,
    protein_g: NutrientProgress,
    carbs_g: NutrientProgress,
    fat_g: NutrientProgress,
    fiber_g: NutrientProgress,
    /// Weight progress (no percent field).
    weight: WeightProgress,
    /// Fasting Window for the query date (fractional hours), derived from
    /// Meals — see `crate::fasting`. Omitted when the date has no Meals or
    /// no Meal exists after it.
    #[serde(skip_serializing_if = "Option::is_none", rename = "fasting_hours")]
    fasting_hours: Option<f64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct WeightProgress {
    #[serde(skip_serializing_if = "Option::is_none", rename = "latest_weight")]
    latest_weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "target_weight")]
    target_weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) remaining: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<ProgressStatus>,
}

/// Active goal row from database.
#[derive(Debug, Clone)]
struct ActiveGoal {
    calories: Option<f64>,
    calories_direction: Option<String>,
    protein_g: Option<f64>,
    protein_g_direction: Option<String>,
    carbs_g: Option<f64>,
    carbs_g_direction: Option<String>,
    fat_g: Option<f64>,
    fat_g_direction: Option<String>,
    fiber_g: Option<f64>,
    fiber_g_direction: Option<String>,
    target_weight: Option<f64>,
}

/// Fetch the active goal as-of a given date.
async fn fetch_active_goal(
    conn: &Connection,
    as_of_date: &str,
) -> Result<Option<ActiveGoal>, ErrorData> {
    let sql = r#"
        SELECT id, effective_from, calories, calories_direction,
               protein_g, protein_g_direction,
               carbs_g, carbs_g_direction,
               fat_g, fat_g_direction,
               fiber_g, fiber_g_direction,
               target_weight
        FROM goals
        WHERE effective_from <= ?
        ORDER BY effective_from DESC
        LIMIT 1
    "#;
    let mut stmt = conn
        .prepare(sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut rows = stmt
        .query((as_of_date,))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

    match rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
    {
        // Columns: id(0), effective_from(1), calories(2), calories_direction(3),
        //          protein_g(4), protein_g_direction(5),
        //          carbs_g(6), carbs_g_direction(7),
        //          fat_g(8), fat_g_direction(9),
        //          fiber_g(10), fiber_g_direction(11),
        //          target_weight(12)
        Some(row) => {
            let get_opt_f64 = |idx: usize| -> Option<f64> {
                row.get_value(idx).ok().and_then(|v| match v {
                    turso::Value::Real(r) => Some(r),
                    turso::Value::Null => None,
                    _ => None,
                })
            };
            let get_opt_str = |idx: usize| -> Option<String> {
                row.get_value(idx).ok().and_then(|v| match v {
                    turso::Value::Text(s) => Some(s),
                    turso::Value::Null => None,
                    _ => None,
                })
            };

            Ok(Some(ActiveGoal {
                // Skip columns 0 (id) and 1 (effective_from) — not needed for progress
                calories: get_opt_f64(2),
                calories_direction: get_opt_str(3),
                protein_g: get_opt_f64(4),
                protein_g_direction: get_opt_str(5),
                carbs_g: get_opt_f64(6),
                carbs_g_direction: get_opt_str(7),
                fat_g: get_opt_f64(8),
                fat_g_direction: get_opt_str(9),
                fiber_g: get_opt_f64(10),
                fiber_g_direction: get_opt_str(11),
                target_weight: get_opt_f64(12),
            }))
        }
        None => Ok(None),
    }
}

/// Aggregate meal totals for a given date.
async fn fetch_consumed_totals(
    conn: &Connection,
    date: &str,
) -> Result<(f64, f64, f64, f64, f64), ErrorData> {
    let sql = r#"
        SELECT
            COALESCE(SUM(total_calories), 0.0),
            COALESCE(SUM(total_protein_g), 0.0),
            COALESCE(SUM(total_carbs_g), 0.0),
            COALESCE(SUM(total_fat_g), 0.0),
            COALESCE(SUM(total_fiber_g), 0.0)
        FROM meals
        WHERE logged_date = ?
    "#;
    let mut stmt = conn
        .prepare(sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut rows = stmt
        .query((date,))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

    match rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
    {
        Some(row) => Ok((
            row.get::<f64>(0).unwrap_or(0.0),
            row.get::<f64>(1).unwrap_or(0.0),
            row.get::<f64>(2).unwrap_or(0.0),
            row.get::<f64>(3).unwrap_or(0.0),
            row.get::<f64>(4).unwrap_or(0.0),
        )),
        None => Ok((0.0, 0.0, 0.0, 0.0, 0.0)),
    }
}

/// Fetch latest weight entry as-of a given date.
async fn fetch_latest_weight(
    conn: &Connection,
    as_of_date: &str,
) -> Result<Option<f64>, ErrorData> {
    let sql = r#"
        SELECT value FROM weight_entries
        WHERE logged_date <= ?
        ORDER BY logged_date DESC
        LIMIT 1
    "#;
    let mut stmt = conn
        .prepare(sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut rows = stmt
        .query((as_of_date,))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

    match rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
    {
        Some(row) => {
            let value = row.get_value(0).map_err(|e| {
                ErrorData::storage_failure(format!("failed to read weight value: {e}"))
            })?;
            Ok(match value {
                turso::Value::Real(r) => Some(r),
                turso::Value::Null => None,
                _ => None,
            })
        }
        None => Ok(None),
    }
}

/// Compute per-nutrient progress fields.
pub(crate) fn nutrient_progress(
    consumed: f64,
    target: Option<f64>,
    direction: Option<Direction>,
) -> NutrientProgress {
    let (remaining, percent, status) = if let Some(t) = target {
        let rem = t - consumed;
        let pct = if t == 0.0 {
            None
        } else {
            Some((consumed / t) * 100.0)
        };
        let st = if (rem - 0.0).abs() < 1e-9 {
            Some(ProgressStatus::Met)
        } else if rem > 0.0 {
            Some(ProgressStatus::Under)
        } else {
            Some(ProgressStatus::Over)
        };
        (Some(rem), pct, st)
    } else {
        (None, None, None)
    };

    NutrientProgress {
        consumed,
        target,
        remaining,
        percent,
        direction,
        status,
    }
}

/// Compute weight progress fields.
pub(crate) fn weight_progress(
    latest_weight: Option<f64>,
    target_weight: Option<f64>,
) -> WeightProgress {
    let (remaining, status) = match (latest_weight, target_weight) {
        (Some(lw), Some(tw)) => {
            let rem = tw - lw;
            let st = if (rem - 0.0).abs() < 1e-9 {
                Some(ProgressStatus::Met)
            } else if rem > 0.0 {
                Some(ProgressStatus::Under)
            } else {
                Some(ProgressStatus::Over)
            };
            (Some(rem), st)
        }
        _ => (None, None),
    };

    WeightProgress {
        latest_weight,
        target_weight,
        remaining,
        status,
    }
}

// ---------------------------------------------------------------------------
// SetNutritionGoals Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct SetNutritionGoalsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calories: Option<f64>,
    #[serde(rename = "calories_direction", skip_serializing_if = "Option::is_none")]
    pub calories_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protein_g: Option<f64>,
    #[serde(
        rename = "protein_g_direction",
        skip_serializing_if = "Option::is_none"
    )]
    pub protein_g_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carbs_g: Option<f64>,
    #[serde(rename = "carbs_g_direction", skip_serializing_if = "Option::is_none")]
    pub carbs_g_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fat_g: Option<f64>,
    #[serde(rename = "fat_g_direction", skip_serializing_if = "Option::is_none")]
    pub fat_g_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fiber_g: Option<f64>,
    #[serde(rename = "fiber_g_direction", skip_serializing_if = "Option::is_none")]
    pub fiber_g_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "target_weight")]
    pub target_weight: Option<f64>,
}

pub struct SetNutritionGoals {
    clock: Clock,
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl SetNutritionGoals {
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
impl Operation for SetNutritionGoals {
    fn name(&self) -> &str {
        "set_nutrition_goals"
    }

    fn description(&self) -> &str {
        "Set or update nutrition goals. Partial patch: only provided nutrients are changed; others carry forward from the current active goal. Direction is required the first time a nutrient is set."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(SetNutritionGoalsRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: SetNutritionGoalsRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path).await?
        } else {
            Connection::open().await?
        };

        #[cfg(not(test))]
        let conn = Connection::open().await?;

        let today_str = Clock::format_date(self.clock.today());

        // Fetch current active goal for carry-forward
        let prior = fetch_active_goal(&conn, &today_str).await?;

        // Determine directions: if prior goal exists, carry forward missing ones.
        // If a nutrient value is being newly set (not in prior), direction is required.
        // Also validates that a provided direction string is one of the known values.
        let validate_and_resolve_direction = |nutrient_name: &str,
                                              value_is_set: bool,
                                              provided_dir: Option<&String>,
                                              prior_dir: Option<&String>|
         -> Result<Option<String>, ErrorData> {
            const VALID_DIRECTIONS: [&str; 3] = ["target", "minimum", "maximum"];
            if let Some(d) = provided_dir
                && !VALID_DIRECTIONS.contains(&d.as_str())
            {
                return Err(ErrorData::validation(
                    format!("{nutrient_name}_direction"),
                    format!("must be one of 'target', 'minimum', 'maximum', got '{d}'"),
                ));
            }
            if !value_is_set {
                // Not setting this nutrient; return prior direction (for carry-forward)
                return Ok(prior_dir.cloned());
            }
            // Value is being set
            if let Some(d) = provided_dir {
                Ok(Some(d.clone()))
            } else if let Some(d) = prior_dir {
                // Carrying forward existing direction
                Ok(Some(d.clone()))
            } else {
                // New nutrient without direction — error
                Err(ErrorData::validation(
                    format!("{nutrient_name}_direction"),
                    "required when setting a nutrient target for the first time",
                ))
            }
        };

        // Build merged values: new overrides prior, prior fills gaps.
        let merge =
            |new_val: Option<f64>, prior_val: Option<f64>| -> Option<f64> { new_val.or(prior_val) };

        // One entry per directional nutrient: (name, new value, provided direction,
        // prior value, prior direction). target_weight has no direction and is
        // merged separately below.
        let nutrients: [(
            &str,
            Option<f64>,
            Option<&String>,
            Option<f64>,
            Option<&String>,
        ); 5] = [
            (
                "calories",
                req.calories,
                req.calories_direction.as_ref(),
                prior.as_ref().and_then(|g| g.calories),
                prior.as_ref().and_then(|g| g.calories_direction.as_ref()),
            ),
            (
                "protein_g",
                req.protein_g,
                req.protein_g_direction.as_ref(),
                prior.as_ref().and_then(|g| g.protein_g),
                prior.as_ref().and_then(|g| g.protein_g_direction.as_ref()),
            ),
            (
                "carbs_g",
                req.carbs_g,
                req.carbs_g_direction.as_ref(),
                prior.as_ref().and_then(|g| g.carbs_g),
                prior.as_ref().and_then(|g| g.carbs_g_direction.as_ref()),
            ),
            (
                "fat_g",
                req.fat_g,
                req.fat_g_direction.as_ref(),
                prior.as_ref().and_then(|g| g.fat_g),
                prior.as_ref().and_then(|g| g.fat_g_direction.as_ref()),
            ),
            (
                "fiber_g",
                req.fiber_g,
                req.fiber_g_direction.as_ref(),
                prior.as_ref().and_then(|g| g.fiber_g),
                prior.as_ref().and_then(|g| g.fiber_g_direction.as_ref()),
            ),
        ];

        let mut merged_values: [Option<f64>; 5] = [None; 5];
        let mut resolved_dirs: [Option<String>; 5] = [None, None, None, None, None];
        for (i, (name, value, provided_dir, prior_value, prior_dir)) in
            nutrients.into_iter().enumerate()
        {
            resolved_dirs[i] =
                validate_and_resolve_direction(name, value.is_some(), provided_dir, prior_dir)?;
            merged_values[i] = merge(value, prior_value);
        }
        let [
            merged_calories,
            merged_protein_g,
            merged_carbs_g,
            merged_fat_g,
            merged_fiber_g,
        ] = merged_values;
        let [cal_dir, prot_dir, carbs_dir, fat_dir, fiber_dir] = resolved_dirs;

        let merged_target_weight = merge(
            req.target_weight,
            prior.as_ref().and_then(|g| g.target_weight),
        );

        // Insert new goal row
        let sql = r#"
            INSERT INTO goals (
                effective_from,
                calories, calories_direction,
                protein_g, protein_g_direction,
                carbs_g, carbs_g_direction,
                fat_g, fat_g_direction,
                fiber_g, fiber_g_direction,
                target_weight
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING id
        "#;

        let params: (
            &str,
            Option<f64>,
            Option<&str>,
            Option<f64>,
            Option<&str>,
            Option<f64>,
            Option<&str>,
            Option<f64>,
            Option<&str>,
            Option<f64>,
            Option<&str>,
            Option<f64>,
        ) = (
            &today_str,
            merged_calories,
            cal_dir.as_deref(),
            merged_protein_g,
            prot_dir.as_deref(),
            merged_carbs_g,
            carbs_dir.as_deref(),
            merged_fat_g,
            fat_dir.as_deref(),
            merged_fiber_g,
            fiber_dir.as_deref(),
            merged_target_weight,
        );

        let mut stmt = conn
            .prepare(sql)
            .await
            .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
        let mut rows = stmt
            .query(params)
            .await
            .map_err(|e| ErrorData::storage_failure(format!("insert failed: {e}")))?;

        let goal_id = match rows
            .next()
            .await
            .map_err(|e| ErrorData::storage_failure(format!("failed to read result: {e}")))?
        {
            Some(row) => match row
                .get_value(0)
                .map_err(|e| ErrorData::storage_failure(format!("failed to read goal_id: {e}")))?
            {
                turso::Value::Integer(id) => id,
                other => {
                    return Err(ErrorData::storage_failure(format!(
                        "unexpected value type for goal_id: {:?}",
                        other
                    )));
                }
            },
            None => {
                return Err(ErrorData::storage_failure(
                    "insert returned no row".to_string(),
                ));
            }
        };

        // Build response
        Ok(serde_json::json!({
            "goal_id": goal_id,
            "effective_from": today_str,
            "calories": merged_calories,
            "calories_direction": cal_dir,
            "protein_g": merged_protein_g,
            "protein_g_direction": prot_dir,
            "carbs_g": merged_carbs_g,
            "carbs_g_direction": carbs_dir,
            "fat_g": merged_fat_g,
            "fat_g_direction": fat_dir,
            "fiber_g": merged_fiber_g,
            "fiber_g_direction": fiber_dir,
            "target_weight": merged_target_weight,
        }))
    }
}

// ---------------------------------------------------------------------------
// GetGoalProgress Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct GetGoalProgressRequest {
    /// Date in YYYY-MM-DD format. Defaults to today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

pub struct GetGoalProgress {
    clock: Clock,
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl GetGoalProgress {
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
impl Operation for GetGoalProgress {
    fn name(&self) -> &str {
        "get_goal_progress"
    }

    fn description(&self) -> &str {
        "Get goal progress for a specific date (defaults to today). Returns per-nutrient consumed vs target comparison and weight progress."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(GetGoalProgressRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: GetGoalProgressRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path).await?
        } else {
            Connection::open().await?
        };

        #[cfg(not(test))]
        let conn = Connection::open().await?;

        // Resolve query date. Reject anything that isn't a strict ISO
        // YYYY-MM-DD date: this value is echoed back verbatim in the response
        // and interpolated into the goal-progress widget's HTML, so malformed
        // input must never reach that far.
        let query_date = match &req.date {
            Some(d) => {
                d.parse::<chrono::NaiveDate>().map_err(|_| {
                    ErrorData::validation(
                        "date",
                        format!("date must be in YYYY-MM-DD format, got: {d}"),
                    )
                })?;
                d.clone()
            }
            None => Clock::format_date(self.clock.today()),
        };

        // Fetch active goal as-of query date
        let goal = fetch_active_goal(&conn, &query_date).await?;

        // Fetch consumed totals for the date
        let (cal, prot, carbs, fat, fiber) = fetch_consumed_totals(&conn, &query_date).await?;

        // Fetch latest weight as-of query date
        let latest_weight = fetch_latest_weight(&conn, &query_date).await?;

        // Fetch the Fasting Window for the query date (last Meal of the day
        // -> next Meal on any later day); None when undefined.
        let fasting_windows =
            crate::fasting::fetch_fasting_windows(&conn, &query_date, &query_date).await?;
        let fasting_hours = fasting_windows.first().map(|w| w.hours);

        // Parse directions from goal
        let parse_direction = |s: Option<&String>| -> Option<Direction> {
            s.and_then(|d| match d.as_str() {
                "target" => Some(Direction::Target),
                "minimum" => Some(Direction::Minimum),
                "maximum" => Some(Direction::Maximum),
                _ => None,
            })
        };

        let goal_target_weight = goal.as_ref().and_then(|g| g.target_weight);

        // One entry per nutrient: (target value, direction string, consumed amount).
        let nutrients: [(Option<f64>, Option<&String>, f64); 5] = [
            (
                goal.as_ref().and_then(|g| g.calories),
                goal.as_ref().and_then(|g| g.calories_direction.as_ref()),
                cal,
            ),
            (
                goal.as_ref().and_then(|g| g.protein_g),
                goal.as_ref().and_then(|g| g.protein_g_direction.as_ref()),
                prot,
            ),
            (
                goal.as_ref().and_then(|g| g.carbs_g),
                goal.as_ref().and_then(|g| g.carbs_g_direction.as_ref()),
                carbs,
            ),
            (
                goal.as_ref().and_then(|g| g.fat_g),
                goal.as_ref().and_then(|g| g.fat_g_direction.as_ref()),
                fat,
            ),
            (
                goal.as_ref().and_then(|g| g.fiber_g),
                goal.as_ref().and_then(|g| g.fiber_g_direction.as_ref()),
                fiber,
            ),
        ];

        // Build nutrient progress
        let mut progress = nutrients.into_iter().map(|(target, dir_str, consumed)| {
            nutrient_progress(consumed, target, parse_direction(dir_str))
        });
        let calories_progress = progress.next().unwrap();
        let protein_g_progress = progress.next().unwrap();
        let carbs_g_progress = progress.next().unwrap();
        let fat_g_progress = progress.next().unwrap();
        let fiber_g_progress = progress.next().unwrap();

        // Build weight progress
        let weight_progress = weight_progress(latest_weight, goal_target_weight);

        Ok(serde_json::to_value(GoalProgress {
            date: query_date,
            calories: calories_progress,
            protein_g: protein_g_progress,
            carbs_g: carbs_g_progress,
            fat_g: fat_g_progress,
            fiber_g: fiber_g_progress,
            weight: weight_progress,
            fasting_hours,
        })
        .map_err(|e| ErrorData::storage_failure(format!("serialization failed: {e}")))?)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCategory;
    use crate::storage::test::TempDb;

    fn clock() -> Clock {
        Clock { tz: chrono_tz::UTC }
    }

    // ---- Pure unit tests: nutrient_progress / weight_progress (no I/O) ----

    #[test]
    fn test_nutrient_progress_no_target() {
        let p = nutrient_progress(150.0, None, Some(Direction::Target));
        assert_eq!(p.consumed, 150.0);
        assert!(p.target.is_none());
        assert!(p.remaining.is_none());
        assert!(p.percent.is_none());
        assert!(p.status.is_none());
        // Direction is passed through unchanged even with no target.
        assert_eq!(p.direction, Some(Direction::Target));
    }

    #[test]
    fn test_nutrient_progress_under_target() {
        let p = nutrient_progress(60.0, Some(100.0), Some(Direction::Minimum));
        assert_eq!(p.target, Some(100.0));
        assert_eq!(p.remaining, Some(40.0));
        assert_eq!(p.percent, Some(60.0));
        assert_eq!(p.status, Some(ProgressStatus::Under));
        assert_eq!(p.direction, Some(Direction::Minimum));
    }

    #[test]
    fn test_nutrient_progress_over_target() {
        let p = nutrient_progress(120.0, Some(100.0), Some(Direction::Maximum));
        assert_eq!(p.remaining, Some(-20.0));
        assert_eq!(p.percent, Some(120.0));
        assert_eq!(p.status, Some(ProgressStatus::Over));
        assert_eq!(p.direction, Some(Direction::Maximum));
    }

    #[test]
    fn test_nutrient_progress_exactly_met() {
        let p = nutrient_progress(100.0, Some(100.0), Some(Direction::Target));
        assert_eq!(p.remaining, Some(0.0));
        assert_eq!(p.percent, Some(100.0));
        assert_eq!(p.status, Some(ProgressStatus::Met));
    }

    /// The implementation treats a difference smaller than the 1e-9 epsilon
    /// as "met" rather than "under"/"over" — verify the boundary directly
    /// rather than relying on exact floating-point equality.
    #[test]
    fn test_nutrient_progress_epsilon_boundary_counts_as_met() {
        let p = nutrient_progress(99.999_999_999_5, Some(100.0), Some(Direction::Target));
        assert_eq!(p.status, Some(ProgressStatus::Met));
    }

    #[test]
    fn test_nutrient_progress_zero_target_guards_div_by_zero() {
        let p = nutrient_progress(10.0, Some(0.0), None);
        // percent is None (would otherwise divide by zero)...
        assert!(p.percent.is_none());
        // ...but remaining/status are still computed normally.
        assert_eq!(p.remaining, Some(-10.0));
        assert_eq!(p.status, Some(ProgressStatus::Over));
        assert!(p.direction.is_none());
    }

    #[test]
    fn test_nutrient_progress_direction_variants_pass_through() {
        for dir in [Direction::Target, Direction::Minimum, Direction::Maximum] {
            let p = nutrient_progress(50.0, Some(50.0), Some(dir.clone()));
            assert_eq!(p.direction, Some(dir));
        }
    }

    #[test]
    fn test_weight_progress_both_none() {
        let p = weight_progress(None, None);
        assert!(p.remaining.is_none());
        assert!(p.status.is_none());
    }

    #[test]
    fn test_weight_progress_only_latest() {
        let p = weight_progress(Some(180.0), None);
        assert!(p.remaining.is_none());
        assert!(p.status.is_none());
    }

    #[test]
    fn test_weight_progress_only_target() {
        let p = weight_progress(None, Some(170.0));
        assert!(p.remaining.is_none());
        assert!(p.status.is_none());
    }

    #[test]
    fn test_weight_progress_status_over() {
        // latest_weight (180) is above target_weight (170): remaining = tw -
        // lw is negative, so status is "over" — current weight exceeds target.
        let p = weight_progress(Some(180.0), Some(170.0));
        assert_eq!(p.remaining, Some(-10.0));
        assert_eq!(p.status, Some(ProgressStatus::Over));
    }

    #[test]
    fn test_weight_progress_status_under() {
        // latest_weight (165) is below target_weight (170): remaining is
        // positive, so status is "under" — still short of the target.
        let p = weight_progress(Some(165.0), Some(170.0));
        assert_eq!(p.remaining, Some(5.0));
        assert_eq!(p.status, Some(ProgressStatus::Under));
    }

    #[test]
    fn test_weight_progress_exactly_met() {
        let p = weight_progress(Some(170.0), Some(170.0));
        assert_eq!(p.remaining, Some(0.0));
        assert_eq!(p.status, Some(ProgressStatus::Met));
    }

    // ---- SetNutritionGoals tests (AC #1) ----

    #[serial_test::serial]
    #[tokio::test]
    async fn test_set_nutrition_goals_first_call_with_direction() {
        let db = TempDb::new().await;
        let clock = clock();
        let op = SetNutritionGoals::new(clock).with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "calories": 2000,
                "calories_direction": "target",
                "protein_g": 150,
                "protein_g_direction": "minimum"
            })))
            .await
            .unwrap();

        assert!(result["goal_id"].is_i64());
        assert!(result["effective_from"].is_string());
        assert_eq!(result["calories"].as_f64().unwrap(), 2000.0);
        assert_eq!(result["calories_direction"].as_str().unwrap(), "target");
        assert_eq!(result["protein_g"].as_f64().unwrap(), 150.0);
        assert_eq!(result["protein_g_direction"].as_str().unwrap(), "minimum");
        // Unset nutrients should be null
        assert!(result["carbs_g"].is_null());
        assert!(result["fat_g"].is_null());
        assert!(result["fiber_g"].is_null());
        assert!(result["target_weight"].is_null());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_set_nutrition_goals_first_call_missing_direction_errors() {
        let db = TempDb::new().await;
        let clock = clock();
        let op = SetNutritionGoals::new(clock).with_db_path(db.path.clone());

        // Setting calories without direction on first call
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "calories": 2000
            })))
            .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().category, ErrorCategory::Validation);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_set_nutrition_goals_partial_update_carries_forward() {
        let db = TempDb::new().await;
        let clock = clock();

        // First call: set calories with direction
        let op = SetNutritionGoals::new(clock).with_db_path(db.path.clone());
        let result1 = op
            .execute_json(Arc::new(serde_json::json!({
                "calories": 2000,
                "calories_direction": "target"
            })))
            .await
            .unwrap();
        assert!(result1["calories"].as_f64().unwrap() == 2000.0);

        // Second call: add protein with direction, omit calories direction (should carry forward)
        let op = SetNutritionGoals::new(clock).with_db_path(db.path.clone());
        let result2 = op
            .execute_json(Arc::new(serde_json::json!({
                "protein_g": 150,
                "protein_g_direction": "minimum"
            })))
            .await
            .unwrap();

        // Calories should carry forward from prior goal
        assert_eq!(result2["calories"].as_f64().unwrap(), 2000.0);
        assert_eq!(result2["calories_direction"].as_str().unwrap(), "target");
        assert_eq!(result2["protein_g"].as_f64().unwrap(), 150.0);
        assert_eq!(result2["protein_g_direction"].as_str().unwrap(), "minimum");
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_set_nutrition_goals_overrides_prior_values() {
        let db = TempDb::new().await;
        let clock = clock();

        // First call
        let op = SetNutritionGoals::new(clock).with_db_path(db.path.clone());
        op.execute_json(Arc::new(serde_json::json!({
            "calories": 2000,
            "calories_direction": "target"
        })))
        .await
        .unwrap();

        // Second call: override calories value, keep direction
        let op = SetNutritionGoals::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "calories": 2500
            })))
            .await
            .unwrap();

        assert_eq!(result["calories"].as_f64().unwrap(), 2500.0);
        // Direction carried forward
        assert_eq!(result["calories_direction"].as_str().unwrap(), "target");
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_set_nutrition_goals_target_weight_no_direction() {
        let db = TempDb::new().await;
        let clock = clock();
        let op = SetNutritionGoals::new(clock).with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "target_weight": 75.0
            })))
            .await
            .unwrap();

        assert_eq!(result["target_weight"].as_f64().unwrap(), 75.0);
        // No direction field for target_weight
        assert!(result["target_weight"].is_number());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_set_nutrition_goals_empty_request_creates_minimal_row() {
        let db = TempDb::new().await;
        let clock = clock();
        let op = SetNutritionGoals::new(clock).with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();

        assert!(result["goal_id"].is_i64());
        assert!(result["effective_from"].is_string());
        // All nutrient fields should be null
        assert!(result["calories"].is_null());
        assert!(result["protein_g"].is_null());
    }

    // ---- GetGoalProgress tests (AC #2, #3) ----

    async fn seed_meal(
        conn: &Connection,
        logged_date: &str,
        calories: f64,
        protein: f64,
        carbs: f64,
        fat: f64,
        fiber: f64,
    ) {
        conn.execute(
            "INSERT INTO meals (logged_at, logged_date, total_calories, total_protein_g, total_carbs_g, total_fat_g, total_fiber_g) VALUES (?, ?, ?, ?, ?, ?, ?)",
            (format!("{}T12:00:00Z", logged_date), logged_date, calories, protein, carbs, fat, fiber),
        )
        .await
        .unwrap();
    }

    async fn seed_goal(
        conn: &Connection,
        effective_from: &str,
        calories: f64,
        calories_direction: &str,
    ) {
        conn.execute(
            "INSERT INTO goals (effective_from, calories, calories_direction) VALUES (?, ?, ?)",
            (effective_from, calories, calories_direction),
        )
        .await
        .unwrap();
    }

    async fn seed_weight_entry(conn: &Connection, logged_date: &str, value: f64) {
        conn.execute(
            "INSERT INTO weight_entries (logged_at, logged_date, value) VALUES (?, ?, ?)",
            (format!("{}T08:00:00Z", logged_date), logged_date, value),
        )
        .await
        .unwrap();
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_full_data() {
        let db = TempDb::new().await;
        let clock = clock();

        // Seed data
        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_goal(&conn, "2025-01-01", 2000.0, "target").await;
        seed_meal(&conn, "2025-01-15", 1500.0, 100.0, 200.0, 50.0, 30.0).await;
        seed_weight_entry(&conn, "2025-01-14", 80.0).await;
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-15"
            })))
            .await
            .unwrap();

        // Verify date
        assert_eq!(result["date"].as_str().unwrap(), "2025-01-15");

        // Verify calories progress
        assert_eq!(result["calories"]["consumed"].as_f64().unwrap(), 1500.0);
        assert_eq!(result["calories"]["target"].as_f64().unwrap(), 2000.0);
        assert_eq!(result["calories"]["remaining"].as_f64().unwrap(), 500.0);
        assert_eq!(result["calories"]["percent"].as_f64().unwrap(), 75.0);
        assert_eq!(result["calories"]["direction"].as_str().unwrap(), "target");
        assert_eq!(result["calories"]["status"].as_str().unwrap(), "under");

        // Verify unset nutrients have null target-derived fields but populated consumed
        assert_eq!(result["protein_g"]["consumed"].as_f64().unwrap(), 100.0);
        assert!(result["protein_g"]["target"].is_null());
        assert!(result["protein_g"]["remaining"].is_null());
        assert!(result["protein_g"]["percent"].is_null());
        assert!(result["protein_g"]["status"].is_null());

        // Verify weight progress
        assert_eq!(result["weight"]["latest_weight"].as_f64().unwrap(), 80.0);
        // No target_weight set, so derived fields are null
        assert!(result["weight"]["target_weight"].is_null());
        assert!(result["weight"]["remaining"].is_null());
        assert!(result["weight"]["status"].is_null());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_no_goal_ever_set() {
        let db = TempDb::new().await;
        let clock = clock();

        // Only seed a meal, no goal
        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_meal(&conn, "2025-01-15", 1500.0, 100.0, 200.0, 50.0, 30.0).await;
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-15"
            })))
            .await
            .unwrap();

        // Consumed values still populate from real data
        assert_eq!(result["calories"]["consumed"].as_f64().unwrap(), 1500.0);
        assert_eq!(result["protein_g"]["consumed"].as_f64().unwrap(), 100.0);

        // But all goal-derived fields are null
        assert!(result["calories"]["target"].is_null());
        assert!(result["calories"]["remaining"].is_null());
        assert!(result["calories"]["percent"].is_null());
        assert!(result["calories"]["direction"].is_null());
        assert!(result["calories"]["status"].is_null());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_goal_set_but_no_meals() {
        let db = TempDb::new().await;
        let clock = clock();

        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_goal(&conn, "2025-01-01", 2000.0, "target").await;
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-15"
            })))
            .await
            .unwrap();

        // Consumed zeros
        assert_eq!(result["calories"]["consumed"].as_f64().unwrap(), 0.0);
        // Target fields present
        assert_eq!(result["calories"]["target"].as_f64().unwrap(), 2000.0);
        assert_eq!(result["calories"]["remaining"].as_f64().unwrap(), 2000.0);
        assert_eq!(result["calories"]["percent"].as_f64().unwrap(), 0.0);
        assert_eq!(result["calories"]["status"].as_str().unwrap(), "under");
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_weight_with_target() {
        let db = TempDb::new().await;
        let clock = clock();

        let conn = Connection::open_at(&db.path).await.unwrap();
        // Goal with target_weight
        conn.execute(
            "INSERT INTO goals (effective_from, target_weight) VALUES (?, ?)",
            ("2025-01-01", 70.0),
        )
        .await
        .unwrap();
        seed_weight_entry(&conn, "2025-01-14", 75.0).await;
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-15"
            })))
            .await
            .unwrap();

        assert_eq!(result["weight"]["latest_weight"].as_f64().unwrap(), 75.0);
        assert_eq!(result["weight"]["target_weight"].as_f64().unwrap(), 70.0);
        assert_eq!(result["weight"]["remaining"].as_f64().unwrap(), -5.0);
        assert_eq!(result["weight"]["status"].as_str().unwrap(), "over");
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_weight_no_entries() {
        let db = TempDb::new().await;
        let clock = clock();

        let conn = Connection::open_at(&db.path).await.unwrap();
        conn.execute(
            "INSERT INTO goals (effective_from, target_weight) VALUES (?, ?)",
            ("2025-01-01", 70.0),
        )
        .await
        .unwrap();
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-15"
            })))
            .await
            .unwrap();

        assert!(result["weight"]["latest_weight"].is_null());
        assert_eq!(result["weight"]["target_weight"].as_f64().unwrap(), 70.0);
        assert!(result["weight"]["remaining"].is_null());
        assert!(result["weight"]["status"].is_null());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_as_of_date_resolves_correctly() {
        let db = TempDb::new().await;
        let clock = clock();

        let conn = Connection::open_at(&db.path).await.unwrap();
        // Goal starting 2025-01-10
        seed_goal(&conn, "2025-01-10", 2500.0, "maximum").await;
        // Meal on 2025-01-12
        seed_meal(&conn, "2025-01-12", 1800.0, 0.0, 0.0, 0.0, 0.0).await;
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());

        // Query for 2025-01-12: goal should be active (effective_from <= date)
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-12"
            })))
            .await
            .unwrap();

        assert_eq!(result["calories"]["target"].as_f64().unwrap(), 2500.0);
        assert_eq!(result["calories"]["consumed"].as_f64().unwrap(), 1800.0);
        assert_eq!(result["calories"]["direction"].as_str().unwrap(), "maximum");

        // Query for 2025-01-05: goal not yet active, so no goal-derived fields
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-05"
            })))
            .await
            .unwrap();

        assert!(result["calories"]["target"].is_null());
        assert!(result["calories"]["direction"].is_null());
        // Consumed is zero (no meals on that date)
        assert_eq!(result["calories"]["consumed"].as_f64().unwrap(), 0.0);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_percent_null_when_target_zero() {
        let db = TempDb::new().await;
        let clock = clock();

        let conn = Connection::open_at(&db.path).await.unwrap();
        // Goal with zero target
        conn.execute(
            "INSERT INTO goals (effective_from, calories, calories_direction) VALUES (?, ?, ?)",
            ("2025-01-01", 0.0, "target"),
        )
        .await
        .unwrap();
        seed_meal(&conn, "2025-01-15", 500.0, 0.0, 0.0, 0.0, 0.0).await;
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-15"
            })))
            .await
            .unwrap();

        assert_eq!(result["calories"]["target"].as_f64().unwrap(), 0.0);
        assert!(result["calories"]["percent"].is_null());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_status_met_exact_equality() {
        let db = TempDb::new().await;
        let clock = clock();

        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_goal(&conn, "2025-01-01", 2000.0, "target").await;
        seed_meal(&conn, "2025-01-15", 2000.0, 0.0, 0.0, 0.0, 0.0).await;
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-15"
            })))
            .await
            .unwrap();

        assert_eq!(result["calories"]["consumed"].as_f64().unwrap(), 2000.0);
        assert_eq!(result["calories"]["target"].as_f64().unwrap(), 2000.0);
        assert_eq!(result["calories"]["remaining"].as_f64().unwrap(), 0.0);
        assert_eq!(result["calories"]["status"].as_str().unwrap(), "met");
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_defaults_to_today() {
        let db = TempDb::new().await;
        let clock = clock();
        let today_str = Clock::format_date(clock.today());

        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_goal(&conn, "2025-01-01", 2000.0, "target").await;
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();

        assert_eq!(result["date"].as_str().unwrap(), today_str);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_rejects_malformed_date() {
        let db = TempDb::new().await;
        let clock = clock();
        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "<img src=x onerror=alert(1)>"
            })))
            .await;

        let err = result.unwrap_err();
        assert_eq!(err.category, ErrorCategory::Validation);
        assert_eq!(err.field.as_deref(), Some("date"));
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_accepts_well_formed_date() {
        let db = TempDb::new().await;
        let clock = clock();
        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-15"
            })))
            .await
            .unwrap();

        assert_eq!(result["date"].as_str().unwrap(), "2025-01-15");
    }

    // ---- GetGoalProgress: fasting_hours (TASK-47) ----

    async fn seed_meal_at(conn: &Connection, logged_at: &str, logged_date: &str) {
        conn.execute(
            "INSERT INTO meals (logged_at, logged_date, total_calories) VALUES (?, ?, ?)",
            (logged_at, logged_date, 100.0),
        )
        .await
        .unwrap();
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_fasting_hours_present() {
        let db = TempDb::new().await;
        let clock = clock();

        // Last meal Jan 10 at 23:00Z; next meal Jan 11 at 07:00Z -> 8h fast.
        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_meal_at(&conn, "2025-01-10T12:00:00Z", "2025-01-10").await;
        seed_meal_at(&conn, "2025-01-10T23:00:00Z", "2025-01-10").await;
        seed_meal_at(&conn, "2025-01-11T07:00:00Z", "2025-01-11").await;
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-10"
            })))
            .await
            .unwrap();

        assert!((result["fasting_hours"].as_f64().unwrap() - 8.0).abs() < f64::EPSILON);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_fasting_hours_omitted_without_meals_on_day() {
        let db = TempDb::new().await;
        let clock = clock();

        // Meals exist, but not on the queried day.
        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_meal_at(&conn, "2025-01-11T07:00:00Z", "2025-01-11").await;
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-10"
            })))
            .await
            .unwrap();

        assert!(result.get("fasting_hours").is_none());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_fasting_hours_omitted_when_no_later_meal() {
        let db = TempDb::new().await;
        let clock = clock();

        // Meal on the queried day, but nothing after it — fast still open.
        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_meal_at(&conn, "2025-01-10T20:00:00Z", "2025-01-10").await;
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-10"
            })))
            .await
            .unwrap();

        assert!(result.get("fasting_hours").is_none());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_goal_progress_fasting_hours_multi_day_skip() {
        let db = TempDb::new().await;
        let clock = clock();

        // Last meal Jan 10 at 20:00Z; Jan 11 empty; first meal Jan 12 at 08:00Z
        // -> 36h fast reported for Jan 10.
        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_meal_at(&conn, "2025-01-10T20:00:00Z", "2025-01-10").await;
        seed_meal_at(&conn, "2025-01-12T08:00:00Z", "2025-01-12").await;
        drop(conn);

        let op = GetGoalProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "date": "2025-01-10"
            })))
            .await
            .unwrap();

        assert!((result["fasting_hours"].as_f64().unwrap() - 36.0).abs() < f64::EPSILON);
    }
}

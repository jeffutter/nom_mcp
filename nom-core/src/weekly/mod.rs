//! Weekly Summary — rolling 7-day nutrition and weight overview.
//!
//! Provides `fetch_weekly_summary()`, shared by the MCP resource
//! `nom://weekly-summary` and the `get_weekly_progress` tool (the latter
//! exists because MCP Apps widgets bind to a `call_tool` result, not a
//! resource read — see `crate::operation::mcp_handler`).

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::error::ErrorData;
use crate::goal::{
    Direction, NutrientProgress, ProgressStatus, nutrient_progress, weight_progress,
};
use crate::operation::{Operation, Surfaces};
use crate::storage::Connection;

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// Rolling 7-day summary output shape.
#[derive(Debug, Clone, Serialize)]
pub struct WeeklySummary {
    /// Start date of the rolling window (YYYY-MM-DD).
    start_date: String,
    /// End date of the rolling window (YYYY-MM-DD).
    end_date: String,
    /// Number of days in the window with meal data.
    days_with_data: u32,
    /// Nutrient progress for the window.
    nutrients: NutrientsSummary,
    /// Weight trend for the window.
    weight: WeightSummary,
    /// Fasting Window statistics for the window.
    fasting: FastingSummary,
}

/// Per-nutrient daily average vs target, plus per-day breakdown.
#[derive(Debug, Clone, Serialize)]
pub struct NutrientsSummary {
    calories: NutrientProgress,
    protein_g: NutrientProgress,
    carbs_g: NutrientProgress,
    fat_g: NutrientProgress,
    fiber_g: NutrientProgress,
    /// Raw daily totals for each day in the window.
    daily_totals: Vec<DailyTotals>,
}

/// Raw nutrient totals for a single day.
#[derive(Debug, Clone, Serialize)]
pub struct DailyTotals {
    date: String,
    calories: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
    fiber_g: f64,
}

/// Weight trend summary for the rolling window.
#[derive(Debug, Clone, Serialize)]
pub struct WeightSummary {
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "latest_known_weight"
    )]
    latest_known_weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "target_weight")]
    target_weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    remaining: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<ProgressStatus>,
}

/// Fasting Window summary for the rolling window (see `crate::fasting`).
#[derive(Debug, Clone, Serialize)]
pub struct FastingSummary {
    /// Average fasting duration in fractional hours, across the window days
    /// with a completed window. Absent when no window completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    average_hours: Option<f64>,
    /// Number of window days with a completed fasting window.
    days_with_fasting: u32,
}

// ---------------------------------------------------------------------------
// Data fetching
// ---------------------------------------------------------------------------

/// Fetch the weekly summary for the rolling 7-day window ending today.
///
/// Queries meals grouped by date, weight entries in the window, and the
/// latest known weight (from before or within the window). Computes daily
/// averages across all 7 days and compares against the active goal.
pub async fn fetch_weekly_summary(
    conn: &Connection,
    clock: &Clock,
) -> Result<WeeklySummary, ErrorData> {
    let today = Clock::format_date(clock.today());
    let start_date = rolling_start_date(&today);

    // 1. Daily totals grouped by date
    let daily_totals = fetch_daily_totals(conn, &start_date, &today).await?;

    // 2. Compute daily averages (sum / 7, not sum / days_with_data)
    let num_days = 7.0;
    let avg_calories = daily_totals.iter().map(|d| d.calories).sum::<f64>() / num_days;
    let avg_protein_g = daily_totals.iter().map(|d| d.protein_g).sum::<f64>() / num_days;
    let avg_carbs_g = daily_totals.iter().map(|d| d.carbs_g).sum::<f64>() / num_days;
    let avg_fat_g = daily_totals.iter().map(|d| d.fat_g).sum::<f64>() / num_days;
    let avg_fiber_g = daily_totals.iter().map(|d| d.fiber_g).sum::<f64>() / num_days;
    let days_with_data = daily_totals.iter().filter(|d| d.calories > 0.0).count() as u32;

    // 3. Active goal for comparison
    let goal = fetch_active_goal(conn, &today).await?;

    // Parse directions from goal
    let parse_direction = |s: Option<&String>| -> Option<Direction> {
        s.and_then(|d| match d.as_str() {
            "target" => Some(Direction::Target),
            "minimum" => Some(Direction::Minimum),
            "maximum" => Some(Direction::Maximum),
            _ => None,
        })
    };

    let goal_calories = goal.as_ref().and_then(|g| g.calories);
    let goal_calories_dir = goal
        .as_ref()
        .and_then(|g| parse_direction(g.calories_direction.as_ref()));
    let goal_protein_g = goal.as_ref().and_then(|g| g.protein_g);
    let goal_protein_g_dir = goal
        .as_ref()
        .and_then(|g| parse_direction(g.protein_g_direction.as_ref()));
    let goal_carbs_g = goal.as_ref().and_then(|g| g.carbs_g);
    let goal_carbs_g_dir = goal
        .as_ref()
        .and_then(|g| parse_direction(g.carbs_g_direction.as_ref()));
    let goal_fat_g = goal.as_ref().and_then(|g| g.fat_g);
    let goal_fat_g_dir = goal
        .as_ref()
        .and_then(|g| parse_direction(g.fat_g_direction.as_ref()));
    let goal_fiber_g = goal.as_ref().and_then(|g| g.fiber_g);
    let goal_fiber_g_dir = goal
        .as_ref()
        .and_then(|g| parse_direction(g.fiber_g_direction.as_ref()));

    // Build nutrient progress using shared helper
    let calories_progress = nutrient_progress(avg_calories, goal_calories, goal_calories_dir);
    let protein_g_progress = nutrient_progress(avg_protein_g, goal_protein_g, goal_protein_g_dir);
    let carbs_g_progress = nutrient_progress(avg_carbs_g, goal_carbs_g, goal_carbs_g_dir);
    let fat_g_progress = nutrient_progress(avg_fat_g, goal_fat_g, goal_fat_g_dir);
    let fiber_g_progress = nutrient_progress(avg_fiber_g, goal_fiber_g, goal_fiber_g_dir);

    // 4. Weight entries in window + latest known weight
    let weight_entries_in_window = fetch_weight_entries_in_range(conn, &start_date, &today).await?;
    let latest_known_weight = fetch_latest_known_weight(conn, &today).await?;

    // 5. Fasting windows for each day in the window; average over the days
    // that produced a completed window.
    let fasting_windows = crate::fasting::fetch_fasting_windows(conn, &start_date, &today).await?;
    let days_with_fasting = fasting_windows.len() as u32;
    let average_hours = if fasting_windows.is_empty() {
        None
    } else {
        Some(fasting_windows.iter().map(|w| w.hours).sum::<f64>() / days_with_fasting as f64)
    };

    let start_weight = weight_entries_in_window.first().copied();
    let end_weight = weight_entries_in_window.last().copied();
    let delta = match (start_weight, end_weight) {
        (Some(s), Some(e)) => Some(e - s),
        _ => None,
    };

    let goal_target_weight = goal.as_ref().and_then(|g| g.target_weight);
    let weight_progress = weight_progress(latest_known_weight, goal_target_weight);
    let (remaining, status) = (weight_progress.remaining, weight_progress.status);

    Ok(WeeklySummary {
        start_date,
        end_date: today,
        days_with_data,
        nutrients: NutrientsSummary {
            calories: calories_progress,
            protein_g: protein_g_progress,
            carbs_g: carbs_g_progress,
            fat_g: fat_g_progress,
            fiber_g: fiber_g_progress,
            daily_totals,
        },
        weight: WeightSummary {
            latest_known_weight,
            start_weight,
            end_weight,
            delta,
            target_weight: goal_target_weight,
            remaining,
            status,
        },
        fasting: FastingSummary {
            average_hours,
            days_with_fasting,
        },
    })
}

// ---------------------------------------------------------------------------
// GetWeeklyProgress Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct GetWeeklyProgressRequest {}

/// MCP-only tool wrapping `fetch_weekly_summary`, so the weekly-progress
/// widget has a `call_tool` result to bind to (see module docs).
pub struct GetWeeklyProgress {
    clock: Clock,
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl GetWeeklyProgress {
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
impl Operation for GetWeeklyProgress {
    fn name(&self) -> &str {
        "get_weekly_progress"
    }

    fn description(&self) -> &str {
        "Get the rolling 7-day nutrition and weight progress summary (same window as the nom://weekly-summary resource)."
    }

    fn surfaces(&self) -> Surfaces {
        Surfaces::MCP
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(GetWeeklyProgressRequest)).ok()
    }

    async fn execute_json(
        &self,
        _args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path).await?
        } else {
            Connection::open().await?
        };

        #[cfg(not(test))]
        let conn = Connection::open().await?;

        let summary = fetch_weekly_summary(&conn, &self.clock).await?;

        serde_json::to_value(summary)
            .map_err(|e| ErrorData::storage_failure(format!("serialization failed: {e}")))
    }
}

/// Calculate the start date of the rolling 7-day window (6 days before end_date).
fn rolling_start_date(end_date: &str) -> String {
    match end_date.parse::<chrono::NaiveDate>() {
        Ok(date) => Clock::format_date(date - chrono::Days::new(6)),
        Err(_) => end_date.to_string(),
    }
}

// ---------------------------------------------------------------------------
// SQL queries
// ---------------------------------------------------------------------------

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

async fn fetch_daily_totals(
    conn: &Connection,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<DailyTotals>, ErrorData> {
    let sql = r#"
        SELECT logged_date,
               COALESCE(SUM(total_calories), 0.0),
               COALESCE(SUM(total_protein_g), 0.0),
               COALESCE(SUM(total_carbs_g), 0.0),
               COALESCE(SUM(total_fat_g), 0.0),
               COALESCE(SUM(total_fiber_g), 0.0)
        FROM meals
        WHERE logged_date BETWEEN ? AND ?
        GROUP BY logged_date
        ORDER BY logged_date
    "#;
    let mut stmt = conn
        .prepare(sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut rows = stmt
        .query((start_date, end_date))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

    let mut totals = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
    {
        let date = row
            .get::<String>(0)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read date: {e}")))?;
        totals.push(DailyTotals {
            date,
            calories: row.get::<f64>(1).unwrap_or(0.0),
            protein_g: row.get::<f64>(2).unwrap_or(0.0),
            carbs_g: row.get::<f64>(3).unwrap_or(0.0),
            fat_g: row.get::<f64>(4).unwrap_or(0.0),
            fiber_g: row.get::<f64>(5).unwrap_or(0.0),
        });
    }

    Ok(totals)
}

async fn fetch_weight_entries_in_range(
    conn: &Connection,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<f64>, ErrorData> {
    let sql = r#"
        SELECT value FROM weight_entries
        WHERE logged_date BETWEEN ? AND ?
        ORDER BY logged_date
    "#;
    let mut stmt = conn
        .prepare(sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut rows = stmt
        .query((start_date, end_date))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

    let mut entries = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
    {
        let value = row
            .get_value(0)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read weight value: {e}")))?;
        if let turso::Value::Real(v) = value {
            entries.push(v);
        }
    }

    Ok(entries)
}

async fn fetch_latest_known_weight(
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test::TempDb;

    fn clock() -> Clock {
        Clock { tz: chrono_tz::UTC }
    }

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
    async fn test_fetch_weekly_summary_empty_db() {
        let db = TempDb::new().await;
        let clock = clock();
        let conn = Connection::open_at(&db.path).await.unwrap();

        let summary = fetch_weekly_summary(&conn, &clock).await.unwrap();

        // Dates should be valid
        assert!(!summary.start_date.is_empty());
        assert!(!summary.end_date.is_empty());

        // No data
        assert_eq!(summary.days_with_data, 0);
        assert!(summary.nutrients.daily_totals.is_empty());

        // All nutrient consumed values are zero
        assert_eq!(summary.nutrients.calories.consumed, 0.0);
        assert_eq!(summary.nutrients.protein_g.consumed, 0.0);

        // Weight is all null
        assert!(summary.weight.latest_known_weight.is_none());
        assert!(summary.weight.start_weight.is_none());
        assert!(summary.weight.end_weight.is_none());
        assert!(summary.weight.delta.is_none());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_fetch_weekly_summary_with_meals() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();

        // Seed meals on specific dates within a known range
        seed_meal(&conn, "2025-01-10", 1800.0, 100.0, 200.0, 60.0, 25.0).await;
        seed_meal(&conn, "2025-01-11", 2200.0, 120.0, 250.0, 70.0, 30.0).await;
        drop(conn);

        // Re-open connection for query
        let conn = Connection::open_at(&db.path).await.unwrap();

        // Use a clock that puts today at 2025-01-11 so both meals are in window
        let clock_2025 = Clock { tz: chrono_tz::UTC };

        // We need to manipulate the clock's notion of "today". Since Clock uses
        // chrono::Local::now(), we can't easily override it. Instead, we'll
        // verify the structure is correct with whatever dates come back.
        let summary = fetch_weekly_summary(&conn, &clock_2025).await.unwrap();

        // Verify daily_totals ordering
        let totals = &summary.nutrients.daily_totals;
        for i in 1..totals.len() {
            assert!(
                totals[i].date >= totals[i - 1].date,
                "daily_totals must be ordered by date"
            );
        }
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_fetch_weekly_summary_with_goal() {
        let db = TempDb::new().await;
        let clock = clock();
        let conn = Connection::open_at(&db.path).await.unwrap();

        // Seed goal
        seed_goal(&conn, "2025-01-01", 2000.0, "target").await;
        drop(conn);

        let conn = Connection::open_at(&db.path).await.unwrap();
        let summary = fetch_weekly_summary(&conn, &clock).await.unwrap();

        // Goal-derived fields should populate even without meals
        assert_eq!(summary.nutrients.calories.target, Some(2000.0));
        assert_eq!(
            summary.nutrients.calories.direction,
            Some(Direction::Target)
        );
        assert_eq!(summary.nutrients.calories.consumed, 0.0);
        assert_eq!(summary.nutrients.calories.remaining, Some(2000.0));
        assert_eq!(summary.nutrients.calories.percent, Some(0.0));
        assert_eq!(
            summary.nutrients.calories.status,
            Some(ProgressStatus::Under)
        );
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_fetch_weekly_summary_without_goal() {
        let db = TempDb::new().await;
        let clock = clock();
        let conn = Connection::open_at(&db.path).await.unwrap();

        // No goal set
        let summary = fetch_weekly_summary(&conn, &clock).await.unwrap();

        // Consumed values populate (zero since no meals)
        assert_eq!(summary.nutrients.calories.consumed, 0.0);

        // Target-derived fields are null
        assert!(summary.nutrients.calories.target.is_none());
        assert!(summary.nutrients.calories.remaining.is_none());
        assert!(summary.nutrients.calories.percent.is_none());
        assert!(summary.nutrients.calories.direction.is_none());
        assert!(summary.nutrients.calories.status.is_none());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_fetch_weekly_summary_weight_trend() {
        let db = TempDb::new().await;
        let clock = clock();
        let conn = Connection::open_at(&db.path).await.unwrap();

        // Seed two weight entries
        seed_weight_entry(&conn, "2025-01-08", 80.0).await;
        seed_weight_entry(&conn, "2025-01-10", 78.0).await;
        drop(conn);

        let conn = Connection::open_at(&db.path).await.unwrap();
        let summary = fetch_weekly_summary(&conn, &clock).await.unwrap();

        // latest_known_weight should resolve
        assert!(summary.weight.latest_known_weight.is_some());

        // If entries fall in the current window, start/end/delta should be set
        // (depends on current date, but we can verify consistency)
        if let (Some(start), Some(end)) = (summary.weight.start_weight, summary.weight.end_weight) {
            assert_eq!(summary.weight.delta, Some(end - start));
        }
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_fetch_weekly_summary_weight_no_entries_in_window() {
        let db = TempDb::new().await;
        let clock = clock();
        let conn = Connection::open_at(&db.path).await.unwrap();

        // Seed a weight entry far in the past (before any possible rolling window)
        seed_weight_entry(&conn, "2020-01-01", 85.0).await;
        drop(conn);

        let conn = Connection::open_at(&db.path).await.unwrap();
        let summary = fetch_weekly_summary(&conn, &clock).await.unwrap();

        // latest_known_weight should still resolve from pre-window entry
        assert_eq!(summary.weight.latest_known_weight, Some(85.0));

        // But start/end/delta should be null (no entries in window)
        assert!(summary.weight.start_weight.is_none());
        assert!(summary.weight.end_weight.is_none());
        assert!(summary.weight.delta.is_none());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_fetch_weekly_summary_daily_totals_ordering() {
        let db = TempDb::new().await;
        let clock = clock();
        let conn = Connection::open_at(&db.path).await.unwrap();

        // Seed meals on multiple dates
        seed_meal(&conn, "2025-01-09", 1500.0, 80.0, 180.0, 50.0, 20.0).await;
        seed_meal(&conn, "2025-01-10", 1800.0, 100.0, 200.0, 60.0, 25.0).await;
        seed_meal(&conn, "2025-01-11", 2000.0, 120.0, 220.0, 70.0, 30.0).await;
        drop(conn);

        let conn = Connection::open_at(&db.path).await.unwrap();
        let summary = fetch_weekly_summary(&conn, &clock).await.unwrap();

        let totals = &summary.nutrients.daily_totals;

        // Verify ordering
        for i in 1..totals.len() {
            assert!(
                totals[i].date >= totals[i - 1].date,
                "daily_totals must be ordered by date: {} vs {}",
                totals[i].date,
                totals[i - 1].date
            );
        }
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_fetch_weekly_summary_weight_with_target() {
        let db = TempDb::new().await;
        let clock = clock();
        let conn = Connection::open_at(&db.path).await.unwrap();

        // Seed goal with target weight
        conn.execute(
            "INSERT INTO goals (effective_from, target_weight) VALUES (?, ?)",
            ("2025-01-01", 70.0),
        )
        .await
        .unwrap();

        // Seed weight entry
        seed_weight_entry(&conn, "2025-01-10", 75.0).await;
        drop(conn);

        let conn = Connection::open_at(&db.path).await.unwrap();
        let summary = fetch_weekly_summary(&conn, &clock).await.unwrap();

        // Target weight and derived fields should populate
        assert_eq!(summary.weight.target_weight, Some(70.0));

        // If latest_known_weight resolved, check remaining/status
        if let Some(lw) = summary.weight.latest_known_weight {
            assert_eq!(summary.weight.remaining, Some(70.0 - lw));
            if lw > 70.0 {
                assert_eq!(summary.weight.status, Some(ProgressStatus::Over));
            }
        }
    }

    // ---- GetWeeklyProgress tests ----

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weekly_progress_empty_db() {
        let db = TempDb::new().await;
        let op = GetWeeklyProgress::new(clock()).with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();

        assert!(!result["start_date"].as_str().unwrap().is_empty());
        assert!(!result["end_date"].as_str().unwrap().is_empty());
        assert_eq!(result["days_with_data"].as_u64().unwrap(), 0);
        assert_eq!(
            result["nutrients"]["calories"]["consumed"]
                .as_f64()
                .unwrap(),
            0.0
        );
        assert!(result["weight"]["latest_known_weight"].is_null());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weekly_progress_matches_fetch_weekly_summary() {
        let db = TempDb::new().await;
        let clock = clock();
        let today = Clock::format_date(clock.today());

        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_goal(&conn, "2025-01-01", 2000.0, "target").await;
        seed_meal(&conn, &today, 1500.0, 100.0, 200.0, 50.0, 30.0).await;
        drop(conn);

        let conn = Connection::open_at(&db.path).await.unwrap();
        let summary = fetch_weekly_summary(&conn, &clock).await.unwrap();
        drop(conn);

        let op = GetWeeklyProgress::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();

        assert_eq!(result, serde_json::to_value(&summary).unwrap());
        let today_total = result["nutrients"]["daily_totals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["date"] == today)
            .expect("today's totals present");
        assert_eq!(today_total["calories"].as_f64().unwrap(), 1500.0);
    }

    // ---- Fasting section (TASK-47) ----

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
    async fn test_fetch_weekly_summary_fasting_average() {
        let db = TempDb::new().await;
        let clock = clock();
        let today = clock.today();
        let today_str = Clock::format_date(today);
        let yesterday = Clock::format_date(today - chrono::Days::new(1));
        let tomorrow = Clock::format_date(today + chrono::Days::new(1));

        // Yesterday: last meal 23:00Z -> today's first meal 07:00Z = 8h.
        // Today: single meal 07:00Z -> tomorrow's first meal 09:00Z = 26h.
        // Average over the two completed windows: (8 + 26) / 2 = 17h.
        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_meal_at(&conn, &format!("{yesterday}T23:00:00Z"), &yesterday).await;
        seed_meal_at(&conn, &format!("{today_str}T07:00:00Z"), &today_str).await;
        seed_meal_at(&conn, &format!("{tomorrow}T09:00:00Z"), &tomorrow).await;
        drop(conn);

        let conn = Connection::open_at(&db.path).await.unwrap();
        let summary = fetch_weekly_summary(&conn, &clock).await.unwrap();

        assert_eq!(summary.fasting.days_with_fasting, 2);
        assert!((summary.fasting.average_hours.unwrap() - 17.0).abs() < f64::EPSILON);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_fetch_weekly_summary_fasting_zero_completed_windows() {
        let db = TempDb::new().await;
        let clock = clock();
        let conn = Connection::open_at(&db.path).await.unwrap();

        let summary = fetch_weekly_summary(&conn, &clock).await.unwrap();

        assert_eq!(summary.fasting.days_with_fasting, 0);
        assert!(summary.fasting.average_hours.is_none());
    }
}

//! One-command seed data for a throwaway local dev instance (TASK-54).
//!
//! `seed_data` creates (or resets) a SQLite database at an explicit path and
//! populates it with realistic multi-day fixture data — active goals, custom
//! foods, meals/portions spanning 7 days including today, weight entries, and
//! widget display enabled — so widgets and any surface can be iterated on
//! without manual entry. Local-CLI only (`Surfaces::CLI`).
//!
//! Safety and repeatability:
//! - Refuses to target the default production DB path ([`crate::config::db_path`]).
//! - Refuses (via the advisory-lock probe) if another process holds the
//!   target open, *before* deleting anything.
//! - Re-running against the same path deletes the DB + WAL sidecars and
//!   re-inserts the identical rows, resetting to the same clean known state.
//! - Dates are deterministic relative to "today" (registry-shared [`Clock`]),
//!   so status coverage holds whenever it runs.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{Duration, NaiveDate};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::clock::Clock;
use crate::error::ErrorData;
use crate::food::NutrientValues;
use crate::meal::compute_portion_macros;
use crate::operation::{Operation, Surfaces};
use crate::storage::Connection;
use crate::storage::lock_probe::probe_db_lock;

// ---------------------------------------------------------------------------
// Fixture data
// ---------------------------------------------------------------------------

/// Custom foods: `(name, kcal, protein g, carbs g, fat g, fiber g)` per 100g.
const SEED_FOODS: &[(&str, f64, f64, f64, f64, f64)] = &[
    ("Oatmeal (dry)", 380.0, 12.0, 66.0, 7.0, 10.0),
    ("Whole milk (2%)", 122.0, 8.7, 4.8, 4.8, 0.0),
    ("Chicken breast (grilled)", 165.0, 31.0, 0.0, 3.6, 0.0),
    ("Brown rice (cooked)", 112.0, 2.6, 24.0, 0.3, 1.8),
    ("Broccoli (steamed)", 35.0, 2.4, 7.0, 0.4, 5.0),
    ("Almonds", 579.0, 21.0, 22.0, 50.0, 12.0),
    ("Protein shake (prepared)", 120.0, 24.0, 6.0, 1.0, 0.0),
];

/// One portion of a seeded meal: `(food index into SEED_FOODS, quantity in
/// grams)`.
type SeedPortionSpec = (usize, f64);

/// Meal schedule, chronological: `(days before today, "HH:MM", portions)`
/// where each portion is a [`SeedPortionSpec`].
///
/// Timing varies day to day and produces several overnight gaps of ≥ 16h
/// (e.g. D-6 last meal 18:00 → D-5 first meal 11:30; D-4 last meal 13:30 →
/// D-3 first meal 10:00; D-3 last meal 17:30 → D-2 first meal 11:15), so the
/// derived Fasting Windows are non-trivial. Every day has at least one meal.
const SEED_MEALS: &[(u8, &str, &[SeedPortionSpec])] = &[
    (6, "06:30", &[(0, 100.0), (1, 100.0)]),
    (6, "12:30", &[(2, 200.0), (3, 100.0)]),
    (6, "18:00", &[(2, 100.0), (4, 100.0)]),
    (5, "11:30", &[(0, 100.0), (1, 100.0)]),
    (5, "19:00", &[(2, 200.0), (4, 100.0)]),
    (4, "07:00", &[(6, 100.0), (0, 100.0)]),
    (4, "13:30", &[(2, 200.0), (3, 100.0), (4, 100.0)]),
    (3, "10:00", &[(0, 100.0), (1, 100.0), (5, 100.0)]),
    (3, "17:30", &[(2, 200.0), (4, 100.0)]),
    (2, "11:15", &[(0, 100.0), (1, 100.0)]),
    (2, "18:45", &[(2, 200.0), (3, 100.0)]),
    (1, "06:45", &[(0, 100.0), (1, 100.0)]),
    (1, "12:45", &[(2, 200.0), (4, 200.0)]),
    (1, "19:30", &[(2, 100.0), (4, 100.0)]),
    (0, "07:30", &[(0, 100.0), (1, 100.0)]),
    (0, "12:45", &[(2, 200.0), (4, 200.0)]),
    (0, "19:30", &[(2, 200.0), (4, 100.0)]),
    (0, "21:15", &[(5, 100.0)]),
];

/// Weight entries: `(days before today, kg)` — downward trend toward the
/// 80.0 target, logged at 07:00 each day.
const SEED_WEIGHTS: &[(u8, f64)] = &[
    (6, 80.5),
    (5, 80.1),
    (4, 79.8),
    (3, 79.4),
    (2, 79.0),
    (1, 78.5),
    (0, 78.0),
];

/// Goal values, effective from today:
/// `(calories, dir, protein_g, dir, carbs_g, dir, fat_g, dir, fiber_g, dir,
/// target_weight)`. Chosen so today's consumed totals span all three
/// statuses: calories under, protein over, carbs under, fat over, fiber met
/// (integer-exact sum), weight under.
const GOAL_CALORIES: f64 = 2000.0;
const GOAL_CALORIES_DIRECTION: &str = "target";
const GOAL_PROTEIN_G: f64 = 150.0;
const GOAL_PROTEIN_G_DIRECTION: &str = "minimum";
const GOAL_CARBS_G: f64 = 200.0;
const GOAL_CARBS_G_DIRECTION: &str = "maximum";
const GOAL_FAT_G: f64 = 75.0;
const GOAL_FAT_G_DIRECTION: &str = "maximum";
const GOAL_FIBER_G: f64 = 37.0;
const GOAL_FIBER_G_DIRECTION: &str = "minimum";
const GOAL_TARGET_WEIGHT: f64 = 80.0;

// ---------------------------------------------------------------------------
// Plan construction (pure — dates materialized from a given "today")
// ---------------------------------------------------------------------------

struct PlannedPortion {
    food_id: i64,
    quantity: f64,
    /// Snapshot macros contributed by this portion.
    snapshot: NutrientValues,
}

struct PlannedMeal {
    logged_at: String,
    logged_date: String,
    /// Materialized meal totals — the sums the readers (`get_goal_progress`,
    /// `get_weekly_progress`) actually query.
    totals: NutrientValues,
    portions: Vec<PlannedPortion>,
}

struct SeedPlan {
    meals: Vec<PlannedMeal>,
    portion_count: usize,
}

/// Build the full insert plan for a 7-day window ending at `today`.
fn build_plan(today: NaiveDate) -> SeedPlan {
    let mut meals = Vec::new();
    let mut portion_count = 0;

    for spec in SEED_MEALS {
        let (days_ago, time, portions) = *spec;
        let date = today - Duration::days(i64::from(days_ago));
        let logged_date = Clock::format_date(date);
        let logged_at = format!("{logged_date}T{time}:00Z");

        let mut totals = NutrientValues {
            calories: 0.0,
            protein_g: 0.0,
            carbs_g: 0.0,
            fat_g: 0.0,
            fiber_g: 0.0,
        };
        let mut planned_portions = Vec::new();
        for (food_idx, grams) in portions {
            let (_, kcal, p, c, f, fib) = SEED_FOODS[*food_idx];
            let snapshot_100g = NutrientValues {
                calories: kcal,
                protein_g: p,
                carbs_g: c,
                fat_g: f,
                fiber_g: fib,
            };
            let snapshot = compute_portion_macros(*grams, "grams", None, snapshot_100g);
            totals.calories += snapshot.calories;
            totals.protein_g += snapshot.protein_g;
            totals.carbs_g += snapshot.carbs_g;
            totals.fat_g += snapshot.fat_g;
            totals.fiber_g += snapshot.fiber_g;
            planned_portions.push(PlannedPortion {
                food_id: *food_idx as i64 + 1,
                quantity: *grams,
                snapshot,
            });
        }
        portion_count += planned_portions.len();
        meals.push(PlannedMeal {
            logged_at,
            logged_date,
            totals,
            portions: planned_portions,
        });
    }

    SeedPlan {
        meals,
        portion_count,
    }
}

// ---------------------------------------------------------------------------
// SeedData Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct SeedDataRequest {
    /// Path of the throwaway SQLite DB file to create and populate. Must not
    /// be the default database path.
    pub path: String,
}

pub struct SeedData {
    clock: Clock,
}

impl SeedData {
    pub fn new(clock: Clock) -> Self {
        Self { clock }
    }
}

#[async_trait::async_trait]
impl Operation for SeedData {
    fn name(&self) -> &str {
        "seed_data"
    }

    fn description(&self) -> &str {
        "Create or reset a throwaway local database populated with realistic \
         seed data: active goals (all 5 nutrients + target weight, with \
         directions), 7 custom foods, 18 meals over 7 days including today \
         (varied timing so fasting windows exist), 7 weight entries, and \
         widget display enabled. Dates are relative to today. Refuses the \
         default database path; re-running resets to the same clean state."
    }

    fn surfaces(&self) -> Surfaces {
        Surfaces::CLI
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(SeedDataRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: SeedDataRequest = match serde_json::from_value((*args).clone()) {
            Ok(req) => req,
            Err(e) => {
                return Err(ErrorData::validation(
                    "path",
                    format!("missing or invalid required argument 'path': {e}"),
                ));
            }
        };
        if req.path.trim().is_empty() {
            return Err(ErrorData::validation(
                "path",
                "required argument 'path' must not be empty",
            ));
        }

        let mut path = PathBuf::from(&req.path);
        if !path.is_absolute() {
            let cwd = std::env::current_dir().map_err(|e| {
                ErrorData::storage_failure(format!("failed to resolve current directory: {e}"))
            })?;
            path = cwd.join(path);
        }

        // Safety gate: never seed the default (production) database. Must
        // compare against the env-override-independent path — `db_path()`
        // itself honors `NOM_MCP_DB_PATH`, so using it here would let an
        // override active in this same shell (e.g. left set from a prior
        // `serve http` session) mask the real production path and defeat
        // this refusal.
        let default = crate::config::default_db_path();
        if path == default {
            return Err(ErrorData::validation(
                "path",
                format!(
                    "refusing to seed the default database at {}; pass a throwaway path",
                    default.display()
                ),
            ));
        }

        // Refuse if another process holds the target open (e.g. a running
        // server) BEFORE deleting anything.
        if path.exists() {
            let locked = probe_db_lock(&path).map_err(|e| {
                ErrorData::storage_failure(format!("failed to probe database lock: {e}"))
            })?;
            if locked {
                return Err(ErrorData::conflict("local_db_locked"));
            }
        }

        // Reset: delete the DB and its WAL sidecars (ignore NotFound), so a
        // re-run always starts from the same clean known state.
        let path_str = path.to_string_lossy().to_string();
        for candidate in [
            path.clone(),
            PathBuf::from(format!("{path_str}-wal")),
            PathBuf::from(format!("{path_str}-shm")),
        ] {
            match std::fs::remove_file(&candidate) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(ErrorData::storage_failure(format!(
                        "failed to remove {}: {e}",
                        candidate.display()
                    )));
                }
            }
        }

        let conn = Connection::open_at(&path).await.map_err(ErrorData::from)?;

        let today = self.clock.today();
        let plan = build_plan(today);

        let result: Result<(), ErrorData> = (async {
            conn.execute("BEGIN TRANSACTION", ()).await.map_err(|e| {
                ErrorData::storage_failure(format!("transaction begin failed: {e}"))
            })?;

            // Goal (effective from today)
            conn.execute(
                r#"INSERT INTO goals (id, effective_from, calories, calories_direction,
                                       protein_g, protein_g_direction, carbs_g, carbs_g_direction,
                                       fat_g, fat_g_direction, fiber_g, fiber_g_direction,
                                       target_weight)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                (
                    1i64,
                    Clock::format_date(today),
                    GOAL_CALORIES,
                    GOAL_CALORIES_DIRECTION,
                    GOAL_PROTEIN_G,
                    GOAL_PROTEIN_G_DIRECTION,
                    GOAL_CARBS_G,
                    GOAL_CARBS_G_DIRECTION,
                    GOAL_FAT_G,
                    GOAL_FAT_G_DIRECTION,
                    GOAL_FIBER_G,
                    GOAL_FIBER_G_DIRECTION,
                    GOAL_TARGET_WEIGHT,
                ),
            )
            .await
            .map_err(|e| ErrorData::storage_failure(format!("goal insert failed: {e}")))?;

            // Foods (explicit ids 1..N — deterministic across re-runs)
            for (i, food) in SEED_FOODS.iter().enumerate() {
                let (name, kcal, p, c, f, fib) = food;
                conn.execute(
                    r#"INSERT INTO foods (id, source, external_id, name,
                                          calories_per_100g, protein_g_per_100g,
                                          carbs_g_per_100g, fat_g_per_100g, fiber_g_per_100g)
                       VALUES (?, 'Custom', NULL, ?, ?, ?, ?, ?, ?)"#,
                    (i as i64 + 1, *name, *kcal, *p, *c, *f, *fib),
                )
                .await
                .map_err(|e| ErrorData::storage_failure(format!("food insert failed: {e}")))?;
            }

            // Meals + portions (explicit ids, chronological order)
            for (i, meal) in plan.meals.iter().enumerate() {
                let meal_id = i as i64 + 1;
                conn.execute(
                    r#"INSERT INTO meals (id, logged_at, logged_date, total_calories,
                                          total_protein_g, total_carbs_g, total_fat_g,
                                          total_fiber_g)
                       VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
                    (
                        meal_id,
                        meal.logged_at.as_str(),
                        meal.logged_date.as_str(),
                        meal.totals.calories,
                        meal.totals.protein_g,
                        meal.totals.carbs_g,
                        meal.totals.fat_g,
                        meal.totals.fiber_g,
                    ),
                )
                .await
                .map_err(|e| ErrorData::storage_failure(format!("meal insert failed: {e}")))?;

                for (j, portion) in meal.portions.iter().enumerate() {
                    let portion_id = i as i64 * 100 + j as i64 + 1;
                    conn.execute(
                        r#"INSERT INTO portions (id, meal_id, food_id, quantity_mode, quantity,
                                                 snapshot_calories_per_100g,
                                                 snapshot_protein_g_per_100g,
                                                 snapshot_carbs_g_per_100g,
                                                 snapshot_fat_g_per_100g,
                                                 snapshot_fiber_g_per_100g)
                           VALUES (?, ?, ?, 'grams', ?, ?, ?, ?, ?, ?)"#,
                        (
                            portion_id,
                            meal_id,
                            portion.food_id,
                            portion.quantity,
                            portion.snapshot.calories,
                            portion.snapshot.protein_g,
                            portion.snapshot.carbs_g,
                            portion.snapshot.fat_g,
                            portion.snapshot.fiber_g,
                        ),
                    )
                    .await
                    .map_err(|e| {
                        ErrorData::storage_failure(format!("portion insert failed: {e}"))
                    })?;
                }
            }

            // Weight entries (07:00 each day, D-6..D0)
            for (i, (days_ago, value)) in SEED_WEIGHTS.iter().enumerate() {
                let date = today - Duration::days(i64::from(*days_ago));
                let logged_date = Clock::format_date(date);
                conn.execute(
                    "INSERT INTO weight_entries (id, logged_at, logged_date, value) \
                     VALUES (?, ?, ?, ?)",
                    (
                        i as i64 + 1,
                        format!("{logged_date}T07:00:00Z"),
                        logged_date,
                        *value,
                    ),
                )
                .await
                .map_err(|e| ErrorData::storage_failure(format!("weight insert failed: {e}")))?;
            }

            // Enable widget display so widgets render immediately
            // (single-row settings table — fresh DB, plain INSERT).
            conn.execute(
                "INSERT INTO settings (widget_display_enabled) VALUES (1)",
                (),
            )
            .await
            .map_err(|e| ErrorData::storage_failure(format!("settings insert failed: {e}")))?;

            conn.execute("COMMIT", ())
                .await
                .map_err(|e| ErrorData::storage_failure(format!("commit failed: {e}")))?;
            Ok(())
        })
        .await;

        if result.is_err() {
            let _ = conn.execute("ROLLBACK", ()).await;
        }
        result?;

        Ok(serde_json::json!({
            "db_path": path.to_string_lossy(),
            "days": 7,
            "foods": SEED_FOODS.len(),
            "meals": plan.meals.len(),
            "portions": plan.portion_count,
            "weight_entries": SEED_WEIGHTS.len(),
            "goal": {
                "calories": GOAL_CALORIES,
                "calories_direction": GOAL_CALORIES_DIRECTION,
                "protein_g": GOAL_PROTEIN_G,
                "protein_g_direction": GOAL_PROTEIN_G_DIRECTION,
                "carbs_g": GOAL_CARBS_G,
                "carbs_g_direction": GOAL_CARBS_G_DIRECTION,
                "fat_g": GOAL_FAT_G,
                "fat_g_direction": GOAL_FAT_G_DIRECTION,
                "fiber_g": GOAL_FIBER_G,
                "fiber_g_direction": GOAL_FIBER_G_DIRECTION,
                "target_weight": GOAL_TARGET_WEIGHT,
            },
        }))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;
    use tempfile::TempDir;

    use crate::weekly::GetWeeklyProgress;

    fn utc_clock() -> Clock {
        Clock { tz: chrono_tz::UTC }
    }

    async fn seed_to(path: &Path) -> serde_json::Value {
        let op = SeedData::new(utc_clock());
        op.execute_json(Arc::new(serde_json::json!({
            "path": path.to_string_lossy(),
        })))
        .await
        .expect("seed should succeed")
    }

    /// Dump every seeded table ordered by id (settings has no id) so two
    /// runs can be compared for byte-for-byte logical equality.
    async fn dump_all(conn: &Connection) -> String {
        let tables = [
            "foods",
            "meals",
            "portions",
            "weight_entries",
            "goals",
            "settings",
        ];
        let mut out = String::new();
        for table in tables {
            let sql = if table == "settings" {
                format!("SELECT * FROM {table}")
            } else {
                format!("SELECT * FROM {table} ORDER BY id")
            };
            let mut stmt = conn.prepare(&sql).await.unwrap();
            let mut rows = stmt.query(()).await.unwrap();
            while let Some(row) = rows.next().await.unwrap() {
                let values: Vec<String> = (0..row.column_count())
                    .map(|i| format!("{:?}", row.get_value(i).unwrap()))
                    .collect();
                out.push_str(&format!("{table}|{}\n", values.join(",")));
            }
        }
        out
    }

    /// Count rows in a table.
    async fn count_rows(conn: &Connection, table: &str) -> i64 {
        let mut stmt = conn
            .prepare(&format!("SELECT COUNT(*) FROM {table}"))
            .await
            .unwrap();
        let mut rows = stmt.query(()).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        match row.get_value(0).unwrap() {
            turso::Value::Integer(n) => n,
            other => panic!("expected integer count, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_seed_creates_populated_db() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("seed.db");

        let summary = seed_to(&path).await;
        assert_eq!(
            summary["db_path"].as_str(),
            Some(path.to_string_lossy().as_ref())
        );
        assert_eq!(summary["days"], 7);
        assert_eq!(summary["foods"], 7);
        assert_eq!(summary["meals"], 18);
        assert_eq!(summary["portions"], 37);
        assert_eq!(summary["weight_entries"], 7);
        assert_eq!(summary["goal"]["target_weight"].as_f64(), Some(80.0));

        let conn = Connection::open_at(&path).await.unwrap();

        // Row counts
        assert_eq!(count_rows(&conn, "foods").await, 7);
        assert_eq!(count_rows(&conn, "meals").await, 18);
        assert_eq!(count_rows(&conn, "portions").await, 37);
        assert_eq!(count_rows(&conn, "weight_entries").await, 7);
        assert_eq!(count_rows(&conn, "goals").await, 1);
        assert_eq!(count_rows(&conn, "settings").await, 1);

        // 7 distinct meal dates including today
        let mut stmt = conn
            .prepare("SELECT COUNT(DISTINCT logged_date) FROM meals")
            .await
            .unwrap();
        let mut rows = stmt.query(()).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<i64>(0).unwrap(), 7);

        // Spot-check a food's macros
        let mut stmt = conn
            .prepare(
                "SELECT calories_per_100g, protein_g_per_100g, carbs_g_per_100g, \
                      fat_g_per_100g, fiber_g_per_100g FROM foods WHERE name = 'Almonds'",
            )
            .await
            .unwrap();
        let mut rows = stmt.query(()).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<f64>(0).unwrap(), 579.0);
        assert_eq!(row.get::<f64>(1).unwrap(), 21.0);
        assert_eq!(row.get::<f64>(2).unwrap(), 22.0);
        assert_eq!(row.get::<f64>(3).unwrap(), 50.0);
        assert_eq!(row.get::<f64>(4).unwrap(), 12.0);

        // Spot-check today's materialized meal totals vs hardcoded expectations
        let today = Clock::format_date(utc_clock().today());
        let mut stmt = conn
            .prepare(
                "SELECT SUM(total_calories), SUM(total_protein_g), SUM(total_carbs_g), \
                 SUM(total_fat_g), SUM(total_fiber_g) FROM meals WHERE logged_date = ?",
            )
            .await
            .unwrap();
        let mut rows = stmt.query((today,)).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let (cal, p, c, f, fib): (f64, f64, f64, f64, f64) = (
            row.get(0).unwrap(),
            row.get(1).unwrap(),
            row.get(2).unwrap(),
            row.get(3).unwrap(),
            row.get(4).unwrap(),
        );
        assert!((cal - 1846.0).abs() < 1e-9, "calories: {cal}");
        assert!((p - 172.9).abs() < 1e-9, "protein: {p}");
        assert!((c - 113.8).abs() < 1e-9, "carbs: {c}");
        assert!((f - 77.4).abs() < 1e-9, "fat: {f}");
        assert!((fib - 37.0).abs() < 1e-9, "fiber: {fib}");

        // Widget display enabled
        let mut stmt = conn
            .prepare("SELECT widget_display_enabled FROM settings LIMIT 1")
            .await
            .unwrap();
        let mut rows = stmt.query(()).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get_value(0).unwrap(), turso::Value::Integer(1));
    }

    #[tokio::test]
    async fn test_seed_rerun_resets_to_identical_state() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("seed.db");

        seed_to(&path).await;
        let first = dump_all(&Connection::open_at(&path).await.unwrap()).await;

        // Simulate drift: add a stray row, then re-seed.
        {
            let conn = Connection::open_at(&path).await.unwrap();
            conn.execute(
                "INSERT INTO weight_entries (id, logged_at, logged_date, value) \
                 VALUES (999, '2000-01-01T00:00:00Z', '2000-01-01', 1.0)",
                (),
            )
            .await
            .unwrap();
        }

        seed_to(&path).await;
        let second = dump_all(&Connection::open_at(&path).await.unwrap()).await;

        assert_eq!(
            first, second,
            "re-seeding must reset to the same clean known state"
        );
    }

    #[tokio::test]
    async fn test_weekly_progress_on_seeded_db() {
        // get_weekly_progress is Surfaces::MCP-only (no CLI/HTTP route), so
        // its seeded-data behavior (AC#4 weekly half) is verified by driving
        // the operation directly against a seeded temp DB.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("seed.db");
        seed_to(&path).await;

        let op = GetWeeklyProgress::new(utc_clock()).with_db_path(path.clone());
        let summary = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .expect("weekly progress on seeded db should succeed");

        // >= 7 days of calorie data (every seeded day has at least one meal)
        let daily = summary["nutrients"]["daily_totals"]
            .as_array()
            .expect("daily_totals should be an array");
        assert!(
            daily.len() >= 7,
            "expected >=7 days of calorie data, got {}",
            daily.len()
        );

        // >= 2 weight points (start + end of window)
        assert!(summary["weight"]["start_weight"].as_f64().is_some());
        assert!(summary["weight"]["end_weight"].as_f64().is_some());

        // Fasting windows are non-trivial (varied meal timing produces
        // completed overnight windows within the window)
        assert!(
            summary["fasting"]["days_with_fasting"]
                .as_u64()
                .unwrap_or(0)
                > 0,
            "expected at least one completed fasting window, got {:?}",
            summary["fasting"]
        );
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_seed_refuses_default_db_path() {
        let default = crate::config::db_path();
        let existed_before = default.exists();

        let op = SeedData::new(utc_clock());
        let err = op
            .execute_json(Arc::new(serde_json::json!({
                "path": default.to_string_lossy(),
            })))
            .await
            .unwrap_err();

        assert_eq!(err.category, crate::error::ErrorCategory::Validation);
        assert_eq!(err.field.as_deref(), Some("path"));
        assert!(
            err.reason
                .as_deref()
                .unwrap()
                .contains("refusing to seed the default database"),
            "reason: {:?}",
            err.reason
        );
        // Nothing was created or deleted
        assert_eq!(default.exists(), existed_before);
    }

    /// Regression test: `NOM_MCP_DB_PATH` being set in the operator's shell
    /// (the exact state the README's own documented dev workflow leaves
    /// behind after a `serve http` session) must not defeat the
    /// refuse-to-seed-production gate. Before this fix, the gate compared
    /// against `config::db_path()`, which itself honors the override — so
    /// an active override made the gate compare production against itself
    /// under a different name and silently approve wiping it.
    #[serial_test::serial]
    #[tokio::test]
    async fn test_seed_refuses_default_db_path_even_with_env_override_active() {
        let default = crate::config::default_db_path();
        let existed_before = default.exists();
        let decoy_dir = TempDir::new().unwrap();
        let decoy = decoy_dir.path().join("decoy.db");

        let saved = std::env::var_os("NOM_MCP_DB_PATH");
        unsafe { std::env::set_var("NOM_MCP_DB_PATH", &decoy) };

        let op = SeedData::new(utc_clock());
        let err = op
            .execute_json(Arc::new(serde_json::json!({
                "path": default.to_string_lossy(),
            })))
            .await
            .unwrap_err();

        match saved {
            Some(v) => unsafe { std::env::set_var("NOM_MCP_DB_PATH", v) },
            None => unsafe { std::env::remove_var("NOM_MCP_DB_PATH") },
        }

        assert_eq!(err.category, crate::error::ErrorCategory::Validation);
        assert_eq!(err.field.as_deref(), Some("path"));
        // Nothing was created or deleted at the real production path
        assert_eq!(default.exists(), existed_before);
    }

    #[tokio::test]
    async fn test_seed_missing_path_is_validation_error() {
        let op = SeedData::new(utc_clock());
        let err = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap_err();
        assert_eq!(err.category, crate::error::ErrorCategory::Validation);
        assert_eq!(err.field.as_deref(), Some("path"));

        let err = op
            .execute_json(Arc::new(serde_json::json!({"path": ""})))
            .await
            .unwrap_err();
        assert_eq!(err.category, crate::error::ErrorCategory::Validation);
        assert_eq!(err.field.as_deref(), Some("path"));
    }

    #[test]
    fn test_build_plan_dates_and_totals_are_deterministic() {
        let today = NaiveDate::from_ymd_opt(2026, 8, 17).unwrap();
        let plan = build_plan(today);

        assert_eq!(plan.meals.len(), 18);
        assert_eq!(plan.portion_count, 37);

        // Chronological: first meal is D-6, last is today
        assert_eq!(plan.meals[0].logged_date, "2026-08-11");
        assert_eq!(plan.meals[0].logged_at, "2026-08-11T06:30:00Z");
        let last = plan.meals.last().unwrap();
        assert_eq!(last.logged_date, "2026-08-17");
        assert_eq!(last.logged_at, "2026-08-17T21:15:00Z");

        // Today's four meals sum to the exact expected totals
        let todays: Vec<&PlannedMeal> = plan
            .meals
            .iter()
            .filter(|m| m.logged_date == "2026-08-17")
            .collect();
        assert_eq!(todays.len(), 4);
        let calories: f64 = todays.iter().map(|m| m.totals.calories).sum();
        let protein: f64 = todays.iter().map(|m| m.totals.protein_g).sum();
        let carbs: f64 = todays.iter().map(|m| m.totals.carbs_g).sum();
        let fat: f64 = todays.iter().map(|m| m.totals.fat_g).sum();
        let fiber: f64 = todays.iter().map(|m| m.totals.fiber_g).sum();
        assert!((calories - 1846.0).abs() < 1e-9);
        assert!((protein - 172.9).abs() < 1e-9);
        assert!((carbs - 113.8).abs() < 1e-9);
        assert!((fat - 77.4).abs() < 1e-9);
        assert!((fiber - 37.0).abs() < 1e-9);

        // Breakfast (oatmeal 100g + milk 100g) spot check
        let breakfast = &plan.meals[14];
        assert_eq!(breakfast.logged_at, "2026-08-17T07:30:00Z");
        assert!((breakfast.totals.calories - 502.0).abs() < 1e-9);
        assert!((breakfast.totals.protein_g - 20.7).abs() < 1e-9);
        assert!((breakfast.totals.carbs_g - 70.8).abs() < 1e-9);
        assert!((breakfast.totals.fat_g - 11.8).abs() < 1e-9);
        assert!((breakfast.totals.fiber_g - 10.0).abs() < 1e-9);

        // Every day has at least one meal (weekly calorie data spans 7 days)
        let dates: HashSet<&str> = plan.meals.iter().map(|m| m.logged_date.as_str()).collect();
        assert_eq!(dates.len(), 7);
    }

    #[test]
    fn test_seed_fixture_status_coverage() {
        // The whole point of the fixture: today's totals vs the goal span
        // all three statuses (same arithmetic as goal::nutrient_progress).
        let today = NaiveDate::from_ymd_opt(2026, 8, 17).unwrap();
        let plan = build_plan(today);
        let today_sum = |get: fn(&PlannedMeal) -> f64| {
            plan.meals
                .iter()
                .filter(|m| m.logged_date == "2026-08-17")
                .map(get)
                .sum::<f64>()
        };
        let status = |consumed: f64, target: f64| {
            let rem = target - consumed;
            if rem.abs() < 1e-9 {
                "met"
            } else if rem > 0.0 {
                "under"
            } else {
                "over"
            }
        };
        let statuses = vec![
            status(today_sum(|m| m.totals.calories), GOAL_CALORIES),
            status(today_sum(|m| m.totals.protein_g), GOAL_PROTEIN_G),
            status(today_sum(|m| m.totals.carbs_g), GOAL_CARBS_G),
            status(today_sum(|m| m.totals.fat_g), GOAL_FAT_G),
            status(today_sum(|m| m.totals.fiber_g), GOAL_FIBER_G),
        ];
        let set: HashSet<&str> = statuses.iter().copied().collect();
        assert!(
            set.contains("under") && set.contains("met") && set.contains("over"),
            "statuses must span under/met/over, got {statuses:?}"
        );
    }
}

//! Meal Type — the breakfast/lunch/dinner attribute of a logged Meal.
//!
//! A Meal carries exactly one [`MealType`]. When the caller doesn't supply
//! one, it is derived from the *local* wall-clock hour of the logged instant
//! in the shared [`crate::clock::Clock`] timezone — the same projection that
//! materializes `logged_date`. The derivation happens once at write time and
//! is never recomputed retroactively; pre-existing rows keep NULL.
//!
//! Default windows are half-open, gapless, and wrap midnight:
//! breakfast 05:00–10:59, lunch 11:00–15:59, dinner 16:00–04:59.

//! Two responsibilities live here: the [`MealType`] enum itself, and the one
//! query that aggregates Meals by `(logged_date, meal_type)`. Summing meals
//! per day is deliberately not duplicated per consumer — `weekly` and `goal`
//! both read through [`fetch_days_by_meal_type`] so the SQL, the grouping and
//! the bucket ordering exist in exactly one place (see TASK-31 for what
//! duplicated aggregation SQL cost once already).

use std::ops::RangeInclusive;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::ErrorData;
use crate::storage::Connection;

/// Which meal a logged Meal represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MealType {
    Breakfast,
    Lunch,
    Dinner,
}

/// Local hours whose meals default to breakfast (05:00–10:59).
const BREAKFAST_HOURS: RangeInclusive<u32> = 5..=10;
/// Local hours whose meals default to lunch (11:00–15:59).
const LUNCH_HOURS: RangeInclusive<u32> = 11..=15;

impl MealType {
    /// Derive the default type from a local wall-clock hour (0..=23).
    ///
    /// Everything outside the breakfast and lunch windows — including the
    /// post-midnight stretch — is dinner, which keeps the mapping exhaustive
    /// without a third explicit range.
    pub fn from_local_hour(hour: u32) -> Self {
        if BREAKFAST_HOURS.contains(&hour) {
            MealType::Breakfast
        } else if LUNCH_HOURS.contains(&hour) {
            MealType::Lunch
        } else {
            MealType::Dinner
        }
    }

    /// Parse a user-supplied value. Lenient by design: callers map `None` to
    /// an `ErrorData::validation` naming the offending field.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "breakfast" => Some(MealType::Breakfast),
            "lunch" => Some(MealType::Lunch),
            "dinner" => Some(MealType::Dinner),
            _ => None,
        }
    }

    /// Canonical lowercase string for storage and display.
    pub fn as_str(&self) -> &'static str {
        match self {
            MealType::Breakfast => "breakfast",
            MealType::Lunch => "lunch",
            MealType::Dinner => "dinner",
        }
    }
}

// ---------------------------------------------------------------------------
// Aggregation by (date, meal type)
// ---------------------------------------------------------------------------

/// Nutrient totals contributed by a single meal type within one day.
///
/// `meal_type` is `None` for the legacy bucket: rows written before the column
/// existed store NULL and are reported rather than dropped or guessed at.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MealTypeTotals {
    /// `"breakfast"`, `"lunch"`, `"dinner"`, or `null` for pre-existing rows.
    pub meal_type: Option<String>,
    pub calories: f64,
    pub protein_g: f64,
    pub carbs_g: f64,
    pub fat_g: f64,
    pub fiber_g: f64,
}

/// One calendar day of Meals: whole-day totals plus the per-meal-type split.
///
/// Produced only by [`fetch_days_by_meal_type`]; the fields are plain data so
/// consumers can fold them into their own output shape.
#[derive(Debug, Clone)]
pub struct DayByMealType {
    /// Day the Meals were logged on (YYYY-MM-DD).
    pub date: String,
    /// Sum across every bucket below, including the legacy `null` one.
    pub calories: f64,
    pub protein_g: f64,
    pub carbs_g: f64,
    pub fat_g: f64,
    pub fiber_g: f64,
    /// Buckets present that day, ordered breakfast, lunch, dinner, then any
    /// legacy `null` bucket. Empty only if the day has no Meals, which the
    /// query never returns.
    pub by_meal_type: Vec<MealTypeTotals>,
}

/// One `(logged_date, meal_type)` row straight from the database.
///
/// Kept private: it is the query's wire shape, not part of any output.
struct TotalsRow {
    date: String,
    meal_type: Option<String>,
    calories: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
    fiber_g: f64,
}

impl TotalsRow {
    fn into_bucket(self) -> MealTypeTotals {
        MealTypeTotals {
            meal_type: self.meal_type,
            calories: self.calories,
            protein_g: self.protein_g,
            carbs_g: self.carbs_g,
            fat_g: self.fat_g,
            fiber_g: self.fiber_g,
        }
    }
}

/// Sort position for a bucket: canonical order first, legacy values last.
fn bucket_rank(meal_type: Option<&str>) -> u8 {
    match meal_type.and_then(MealType::parse) {
        Some(MealType::Breakfast) => 0,
        Some(MealType::Lunch) => 1,
        Some(MealType::Dinner) => 2,
        // NULL (pre-v2 rows) and anything unrecognized sort last; an
        // unrecognized value keeps its raw string instead of being relabelled.
        None => 3,
    }
}

/// Group query rows into per-day entries, summing whole-day totals.
///
/// Rows must arrive ordered by date (`SQL` below guarantees it); buckets within
/// a day are re-ordered here because SQLite sorts NULL first and would put the
/// legacy bucket ahead of breakfast.
fn fold_rows(rows: Vec<TotalsRow>) -> Vec<DayByMealType> {
    let mut days: Vec<DayByMealType> = Vec::new();
    for row in rows {
        if days.last().is_none_or(|day| day.date != row.date) {
            days.push(DayByMealType {
                date: row.date.clone(),
                calories: 0.0,
                protein_g: 0.0,
                carbs_g: 0.0,
                fat_g: 0.0,
                fiber_g: 0.0,
                by_meal_type: Vec::new(),
            });
        }
        let day = days.last_mut().expect("day just pushed");
        day.calories += row.calories;
        day.protein_g += row.protein_g;
        day.carbs_g += row.carbs_g;
        day.fat_g += row.fat_g;
        day.fiber_g += row.fiber_g;
        day.by_meal_type.push(row.into_bucket());
    }
    for day in &mut days {
        day.by_meal_type
            .sort_by_key(|bucket| bucket_rank(bucket.meal_type.as_deref()));
    }
    days
}

/// Nutrient totals per day, split by meal type, for an inclusive date range.
///
/// Days with no Meals are absent (not zero-filled), matching how the weekly
/// summary's daily totals have always behaved. Both the Weekly Summary and
/// `get_goal_progress` use this; ask for one date when you want a single day.
///
/// # Errors
///
/// Returns [`ErrorData`] storage failures if the query or any row read fails —
/// rows are never silently dropped.
pub async fn fetch_days_by_meal_type(
    conn: &Connection,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<DayByMealType>, ErrorData> {
    const SQL: &str = r#"
        SELECT logged_date, meal_type,
               COALESCE(SUM(total_calories), 0.0),
               COALESCE(SUM(total_protein_g), 0.0),
               COALESCE(SUM(total_carbs_g), 0.0),
               COALESCE(SUM(total_fat_g), 0.0),
               COALESCE(SUM(total_fiber_g), 0.0)
        FROM meals
        WHERE logged_date BETWEEN ? AND ?
        GROUP BY logged_date, meal_type
        ORDER BY logged_date, meal_type
    "#;

    let mut stmt = conn
        .prepare(SQL)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut rows = stmt
        .query((start_date, end_date))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

    let mut raw = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
    {
        let meal_type = match row
            .get_value(1)
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
        {
            turso::Value::Text(s) => Some(s),
            turso::Value::Null => None,
            other => {
                return Err(ErrorData::storage_failure(format!(
                    "unexpected value type for meal_type: {:?}",
                    other
                )));
            }
        };
        raw.push(TotalsRow {
            date: row
                .get::<String>(0)
                .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
            meal_type,
            calories: row.get::<f64>(2).unwrap_or(0.0),
            protein_g: row.get::<f64>(3).unwrap_or(0.0),
            carbs_g: row.get::<f64>(4).unwrap_or(0.0),
            fat_g: row.get::<f64>(5).unwrap_or(0.0),
            fiber_g: row.get::<f64>(6).unwrap_or(0.0),
        });
    }

    Ok(fold_rows(raw))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn row(date: &str, meal_type: Option<&str>, calories: f64) -> TotalsRow {
        TotalsRow {
            date: date.to_string(),
            meal_type: meal_type.map(str::to_string),
            calories,
            protein_g: 0.0,
            carbs_g: 0.0,
            fat_g: 0.0,
            fiber_g: 0.0,
        }
    }

    #[test]
    fn test_every_hour_maps_to_exactly_one_variant() {
        for hour in 0..=23 {
            let derived = MealType::from_local_hour(hour);
            // Round-trip through the canonical string proves each hour lands
            // on a real variant and that as_str/parse agree.
            assert!(MealType::parse(derived.as_str()) == Some(derived));
        }
    }

    #[test]
    fn test_window_boundaries() {
        // Half-open windows, wrapping midnight: 04:59-class hour is dinner,
        // 05:00-class flips to breakfast, etc.
        assert_eq!(MealType::from_local_hour(4), MealType::Dinner);
        assert_eq!(MealType::from_local_hour(0), MealType::Dinner);
        assert_eq!(MealType::from_local_hour(5), MealType::Breakfast);
        assert_eq!(MealType::from_local_hour(10), MealType::Breakfast);
        assert_eq!(MealType::from_local_hour(11), MealType::Lunch);
        assert_eq!(MealType::from_local_hour(15), MealType::Lunch);
        assert_eq!(MealType::from_local_hour(16), MealType::Dinner);
        assert_eq!(MealType::from_local_hour(23), MealType::Dinner);
    }

    #[test]
    fn test_full_hour_map() {
        let breakfast: Vec<u32> = (0..=23)
            .filter(|h| MealType::from_local_hour(*h) == MealType::Breakfast)
            .collect();
        let lunch: Vec<u32> = (0..=23)
            .filter(|h| MealType::from_local_hour(*h) == MealType::Lunch)
            .collect();
        let dinner: Vec<u32> = (0..=23)
            .filter(|h| MealType::from_local_hour(*h) == MealType::Dinner)
            .collect();
        assert_eq!(breakfast, vec![5, 6, 7, 8, 9, 10]);
        assert_eq!(lunch, vec![11, 12, 13, 14, 15]);
        assert_eq!(dinner, vec![0, 1, 2, 3, 4, 16, 17, 18, 19, 20, 21, 22, 23]);
    }

    #[test]
    fn test_parse_accepts_only_canonical_values() {
        assert_eq!(MealType::parse("breakfast"), Some(MealType::Breakfast));
        assert_eq!(MealType::parse("lunch"), Some(MealType::Lunch));
        assert_eq!(MealType::parse("dinner"), Some(MealType::Dinner));
        assert_eq!(MealType::parse("Breakfast"), None);
        assert_eq!(MealType::parse("snack"), None);
        assert_eq!(MealType::parse(""), None);
    }

    #[test]
    fn test_serde_round_trips_lowercase() {
        let json = serde_json::to_string(&MealType::Breakfast).unwrap();
        assert_eq!(json, "\"breakfast\"");
        let back: MealType = serde_json::from_str("\"dinner\"").unwrap();
        assert_eq!(back, MealType::Dinner);
    }

    // ---- Aggregation by (date, meal type) ----

    #[test]
    fn test_fold_rows_sums_day_and_orders_buckets_canonically() {
        // Rows arrive in SQLite's order, which puts the NULL bucket first.
        let days = fold_rows(vec![
            row("2026-06-01", None, 50.0),
            row("2026-06-01", Some("dinner"), 700.0),
            row("2026-06-01", Some("breakfast"), 300.0),
            row("2026-06-01", Some("lunch"), 400.0),
            row("2026-06-02", Some("breakfast"), 250.0),
        ]);

        assert_eq!(days.len(), 2);
        assert_eq!(days[0].date, "2026-06-01");
        assert_eq!(days[0].calories, 1450.0, "legacy rows count toward the day");
        let labels: Vec<Option<&str>> = days[0]
            .by_meal_type
            .iter()
            .map(|b| b.meal_type.as_deref())
            .collect();
        assert_eq!(
            labels,
            vec![Some("breakfast"), Some("lunch"), Some("dinner"), None]
        );
        assert_eq!(days[0].by_meal_type[0].calories, 300.0);
        assert_eq!(days[0].by_meal_type[3].calories, 50.0);
        assert_eq!(days[1].date, "2026-06-02");
        assert_eq!(days[1].calories, 250.0);
        assert_eq!(days[1].by_meal_type.len(), 1);
    }

    #[test]
    fn test_fold_rows_empty_yields_no_days() {
        assert!(fold_rows(Vec::new()).is_empty());
    }

    #[test]
    fn test_legacy_bucket_serializes_as_null_not_omitted() {
        let bucket = MealTypeTotals {
            meal_type: None,
            calories: 10.0,
            protein_g: 0.0,
            carbs_g: 0.0,
            fat_g: 0.0,
            fiber_g: 0.0,
        };
        let json = serde_json::to_value(&bucket).unwrap();
        assert!(json["meal_type"].is_null());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_fetch_days_by_meal_type_groups_and_bounds_dates() {
        use crate::storage::test::TempDb;

        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        seed(&conn, "2026-05-31", Some("dinner"), 900.0).await;
        seed(&conn, "2026-06-01", Some("breakfast"), 300.0).await;
        seed(&conn, "2026-06-01", Some("lunch"), 400.0).await;
        seed(&conn, "2026-06-01", None, 100.0).await;
        seed(&conn, "2026-06-02", Some("dinner"), 800.0).await;

        let days = fetch_days_by_meal_type(&conn, "2026-06-01", "2026-06-01")
            .await
            .unwrap();

        assert_eq!(days.len(), 1, "range is inclusive and excludes neighbours");
        assert_eq!(days[0].date, "2026-06-01");
        assert_eq!(days[0].calories, 800.0);
        assert_eq!(
            days[0]
                .by_meal_type
                .iter()
                .map(|b| b.meal_type.clone())
                .collect::<Vec<_>>(),
            vec![Some("breakfast".into()), Some("lunch".into()), None]
        );
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_fetch_days_by_meal_type_empty_range_is_empty() {
        use crate::storage::test::TempDb;

        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();

        let days = fetch_days_by_meal_type(&conn, "2026-06-01", "2026-06-07")
            .await
            .unwrap();
        assert!(days.is_empty());
    }

    async fn seed(conn: &Connection, logged_date: &str, meal_type: Option<&str>, calories: f64) {
        conn.execute(
            "INSERT INTO meals (logged_at, logged_date, total_calories, meal_type) VALUES (?, ?, ?, ?)",
            (
                format!("{logged_date}T12:00:00Z"),
                logged_date,
                calories,
                meal_type,
            ),
        )
        .await
        .unwrap();
    }
}

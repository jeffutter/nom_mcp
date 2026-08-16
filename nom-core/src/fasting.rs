//! Fasting Window derivation — intermittent fasting computed from Meal data.
//!
//! A day's **Fasting Window** is the time between that day's last logged
//! Meal and the next logged Meal (the earliest Meal on any later day). It is
//! derived entirely from `meals.logged_at` / `meals.logged_date` — nothing
//! is stored or manually logged. If the following calendar day has no meals,
//! the window extends to the first meal on the next day that has one.
//!
//! Shared by the daily report (`get_goal_progress`, via a single-date range)
//! and the Weekly Summary (`fetch_weekly_summary`, via its rolling 7-day
//! range).

use chrono::{DateTime, Utc};

use crate::error::ErrorData;
use crate::storage::Connection;

/// One completed Fasting Window for a single day.
#[derive(Debug, Clone)]
pub struct FastingWindow {
    /// Date the fast started (YYYY-MM-DD, the day of the last meal).
    pub date: String,
    /// Duration from that day's last Meal to the next Meal, in fractional hours.
    pub hours: f64,
}

/// Fetch the completed Fasting Windows for each day in `[start_date, end_date]`.
///
/// For each day D in the range that has at least one Meal, the window runs
/// from D's last Meal (`MAX(logged_at)` where `logged_date = D`) to the
/// earliest Meal on any later day. Days with no Meals are skipped. A day
/// whose next Meal falls after `end_date` still completes its window via a
/// terminal lookup, so the last day of the range is not silently dropped.
///
/// Returns windows ordered by date (ascending), matching the input range.
pub async fn fetch_fasting_windows(
    conn: &Connection,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<FastingWindow>, ErrorData> {
    // Per-day min/max meal timestamps within the range.
    let sql_range = r#"
        SELECT logged_date, MIN(logged_at), MAX(logged_at)
        FROM meals
        WHERE logged_date BETWEEN ? AND ?
        GROUP BY logged_date
        ORDER BY logged_date
    "#;
    let mut stmt = conn
        .prepare(sql_range)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut rows = stmt
        .query((start_date, end_date))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

    let mut days: Vec<(String, String, String)> = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
    {
        let date = row
            .get::<String>(0)
            .map_err(|e| ErrorData::storage_failure(format!("failed to read date: {e}")))?;
        let min_ts = row.get::<String>(1).map_err(|e| {
            ErrorData::storage_failure(format!("failed to read min timestamp: {e}"))
        })?;
        let max_ts = row.get::<String>(2).map_err(|e| {
            ErrorData::storage_failure(format!("failed to read max timestamp: {e}"))
        })?;
        days.push((date, min_ts, max_ts));
    }

    // Earliest meal strictly after the range — terminal fallback for the
    // last day's window when its next meal lies beyond `end_date`.
    let sql_after = r#"
        SELECT MIN(logged_at) FROM meals WHERE logged_date > ?
    "#;
    let mut stmt = conn
        .prepare(sql_after)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut rows = stmt
        .query((end_date,))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;
    let terminal_min: Option<String> = match rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
    {
        Some(row) => row.get::<String>(0).ok().filter(|v| !v.is_empty()),
        None => None,
    };

    // Resolve each day's window. `days` is sorted ascending, so the next
    // day with meals is always the forward scan target.
    let mut windows = Vec::new();
    for (i, (date, _min_ts, max_ts)) in days.iter().enumerate() {
        let last_meal = parse_timestamp(max_ts)?;
        let next_meal = match days.get(i + 1) {
            Some((_next_date, next_min_ts, _)) => Some(parse_timestamp(next_min_ts)?),
            None => terminal_min.as_deref().map(parse_timestamp).transpose()?,
        };
        if let Some(next_meal) = next_meal {
            let seconds = (next_meal - last_meal).num_seconds();
            windows.push(FastingWindow {
                date: date.clone(),
                hours: seconds as f64 / 3600.0,
            });
        }
    }

    Ok(windows)
}

/// Parse a stored `logged_at` value (ISO 8601 UTC, `%Y-%m-%dT%H:%M:%SZ`).
fn parse_timestamp(ts: &str) -> Result<DateTime<Utc>, ErrorData> {
    DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| {
            ErrorData::storage_failure(format!(
                "malformed logged_at value '{ts}' in meals table: {e}"
            ))
        })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test::TempDb;

    async fn seed_meal(conn: &Connection, logged_at: &str, logged_date: &str) {
        conn.execute(
            "INSERT INTO meals (logged_at, logged_date, total_calories) VALUES (?, ?, ?)",
            (logged_at, logged_date, 100.0),
        )
        .await
        .unwrap();
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_window_between_adjacent_days() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();

        // Two meals on Jan 10 (last at 23:00Z), first meal Jan 11 at 07:00Z
        seed_meal(&conn, "2025-01-10T12:00:00Z", "2025-01-10").await;
        seed_meal(&conn, "2025-01-10T23:00:00Z", "2025-01-10").await;
        seed_meal(&conn, "2025-01-11T07:00:00Z", "2025-01-11").await;
        seed_meal(&conn, "2025-01-11T19:00:00Z", "2025-01-11").await;

        let windows = fetch_fasting_windows(&conn, "2025-01-10", "2025-01-11")
            .await
            .unwrap();

        // Only Jan 10's window completes: no meal exists after Jan 11, so
        // Jan 11's fast is still open and must not be reported.
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].date, "2025-01-10");
        // 23:00Z -> 07:00Z = 8h
        assert!((windows[0].hours - 8.0).abs() < f64::EPSILON);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_last_day_completes_via_terminal_lookup() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();

        seed_meal(&conn, "2025-01-10T23:00:00Z", "2025-01-10").await;
        seed_meal(&conn, "2025-01-11T07:00:00Z", "2025-01-11").await;
        seed_meal(&conn, "2025-01-12T09:00:00Z", "2025-01-12").await;

        // Range ends Jan 11; Jan 11's next meal (Jan 12 09:00Z) lies outside
        // the range but must still complete the window.
        let windows = fetch_fasting_windows(&conn, "2025-01-10", "2025-01-11")
            .await
            .unwrap();

        assert_eq!(windows.len(), 2);
        assert_eq!(windows[1].date, "2025-01-11");
        // 07:00Z Jan 11 -> 09:00Z Jan 12 = 26h
        assert!((windows[1].hours - 26.0).abs() < f64::EPSILON);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_no_meals_on_day_is_skipped() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();

        // Only Jan 11 has meals; Jan 10 is empty.
        seed_meal(&conn, "2025-01-11T08:00:00Z", "2025-01-11").await;
        seed_meal(&conn, "2025-01-12T08:00:00Z", "2025-01-12").await;

        let windows = fetch_fasting_windows(&conn, "2025-01-10", "2025-01-11")
            .await
            .unwrap();

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].date, "2025-01-11");
        assert!((windows[0].hours - 24.0).abs() < f64::EPSILON);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_no_meals_after_day_yields_no_window() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();

        seed_meal(&conn, "2025-01-10T20:00:00Z", "2025-01-10").await;

        let windows = fetch_fasting_windows(&conn, "2025-01-10", "2025-01-10")
            .await
            .unwrap();

        assert!(windows.is_empty());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_multi_day_skip_extends_to_next_meal_ever() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();

        // Last meal Jan 10 at 20:00Z; nothing Jan 11; first meal Jan 12 at 08:00Z.
        seed_meal(&conn, "2025-01-10T20:00:00Z", "2025-01-10").await;
        seed_meal(&conn, "2025-01-12T08:00:00Z", "2025-01-12").await;

        let windows = fetch_fasting_windows(&conn, "2025-01-10", "2025-01-12")
            .await
            .unwrap();

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].date, "2025-01-10");
        // 20:00Z Jan 10 -> 08:00Z Jan 12 = 36h
        assert!((windows[0].hours - 36.0).abs() < f64::EPSILON);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_empty_db_returns_no_windows() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();

        let windows = fetch_fasting_windows(&conn, "2025-01-01", "2025-01-07")
            .await
            .unwrap();

        assert!(windows.is_empty());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_fractional_hours() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();

        // 23:30Z -> 07:45Z = 8h 15m = 8.25h
        seed_meal(&conn, "2025-01-10T23:30:00Z", "2025-01-10").await;
        seed_meal(&conn, "2025-01-11T07:45:00Z", "2025-01-11").await;

        let windows = fetch_fasting_windows(&conn, "2025-01-10", "2025-01-10")
            .await
            .unwrap();

        assert_eq!(windows.len(), 1);
        assert!((windows[0].hours - 8.25).abs() < f64::EPSILON);
    }
}

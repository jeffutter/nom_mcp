//! Weight entry operations — log, update, delete, and query by date.
//!
//! Implements `log_weight`, `update_weight_entry`, `delete_weight_entry`,
//! `get_weight_today`, `get_weight_by_date`, `get_weight_by_date_range`, and
//! `get_weight_trend` per doc-5 §5, §13.
//!
//! Weight entries are simple: no FK relationships, no snapshotting, no computed
//! totals — just raw value storage with temporal handling. All deletes are
//! hard deletes with no undo path.

use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::clock::Clock;
use crate::error::ErrorData;
use crate::operation::{Operation, Surfaces};
use crate::storage::Connection;

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// A weight entry summary returned by query operations.
#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct WeightEntrySummary {
    pub id: i64,
    #[serde(rename = "logged_at")]
    pub logged_at: String,
    #[serde(rename = "logged_date")]
    pub logged_date: String,
    pub value: f64,
}

/// Map a `SELECT id, logged_at, logged_date, value` row to a `WeightEntrySummary`.
fn weight_entry_summary_from_row(row: &turso::Row) -> Result<WeightEntrySummary, ErrorData> {
    Ok(WeightEntrySummary {
        id: row
            .get::<i64>(0)
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
        logged_at: row
            .get::<String>(1)
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
        logged_date: row
            .get::<String>(2)
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
        value: row
            .get::<f64>(3)
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
    })
}

/// Build a WeightEntrySummary from a single DB row.
async fn build_weight_summary(
    conn: &Connection,
    entry_id: i64,
) -> Result<WeightEntrySummary, ErrorData> {
    let sql = "SELECT id, logged_at, logged_date, value FROM weight_entries WHERE id = ?";
    let mut stmt = conn
        .prepare(sql)
        .await
        .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
    let mut rows = stmt
        .query((entry_id,))
        .await
        .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

    match rows
        .next()
        .await
        .map_err(|e| ErrorData::storage_failure(format!("failed to read row: {e}")))?
    {
        Some(row) => weight_entry_summary_from_row(&row),
        None => Err(ErrorData::not_found()),
    }
}

/// Parse an ISO-8601 timestamp string into `(logged_at_str, logged_date_str)`.
fn parse_logged_at(ts: &str, clock: &Clock) -> Result<(String, String), ErrorData> {
    let dt: DateTime<Utc> = ts.parse().map_err(|_| {
        ErrorData::validation(
            "logged_at",
            format!("invalid datetime format: {}. Use ISO 8601 format.", ts),
        )
    })?;
    Ok((
        format!("{}", dt.format("%Y-%m-%dT%H:%M:%SZ")),
        Clock::format_date(clock.logged_date(&dt)),
    ))
}

// ---------------------------------------------------------------------------
// LogWeight Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct LogWeightRequest {
    /// Weight value (any unit — stored as-is).
    pub value: f64,
    /// Optional timestamp override (ISO 8601). Defaults to now.
    #[serde(rename = "logged_at", skip_serializing_if = "Option::is_none")]
    pub logged_at: Option<String>,
}

pub struct LogWeight {
    clock: Clock,
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl LogWeight {
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
impl Operation for LogWeight {
    fn name(&self) -> &str {
        "log_weight"
    }

    fn description(&self) -> &str {
        "Log a weight entry. Value is stored as-is (no unit enforcement). Optional logged_at allows backdating."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(LogWeightRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: LogWeightRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        if req.value <= 0.0 {
            return Err(ErrorData::validation("value", "must be greater than zero"));
        }

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path).await?
        } else {
            Connection::open().await?
        };

        #[cfg(not(test))]
        let conn = Connection::open().await?;

        // Determine logged_at and logged_date
        let (logged_at_str, logged_date_str) = if let Some(ref ts) = req.logged_at {
            parse_logged_at(ts, &self.clock)?
        } else {
            let now = chrono::Utc::now();
            (
                format!("{}", now.format("%Y-%m-%dT%H:%M:%SZ")),
                Clock::format_date(self.clock.today()),
            )
        };

        // Insert weight entry
        let sql = r#"
            INSERT INTO weight_entries (logged_at, logged_date, value)
            VALUES (?, ?, ?)
            RETURNING id
        "#;
        let mut stmt = conn
            .prepare(sql)
            .await
            .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
        let mut rows = stmt
            .query((&logged_at_str[..], &logged_date_str[..], req.value))
            .await
            .map_err(|e| ErrorData::storage_failure(format!("insert failed: {e}")))?;

        let entry_id = match rows
            .next()
            .await
            .map_err(|e| ErrorData::storage_failure(format!("failed to read result: {e}")))?
        {
            Some(row) => {
                let value = row.get_value(0).map_err(|e| {
                    ErrorData::storage_failure(format!("failed to read entry_id: {e}"))
                })?;
                match value {
                    turso::Value::Integer(id) => id,
                    other => {
                        return Err(ErrorData::storage_failure(format!(
                            "unexpected value type for entry_id: {:?}",
                            other
                        )));
                    }
                }
            }
            None => {
                return Err(ErrorData::storage_failure(
                    "insert returned no row".to_string(),
                ));
            }
        };

        Ok(serde_json::json!({
            "entry_id": entry_id,
            "logged_at": logged_at_str,
            "logged_date": logged_date_str,
            "value": req.value,
        }))
    }
}

// ---------------------------------------------------------------------------
// UpdateWeightEntry Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct UpdateWeightEntryRequest {
    /// The weight entry ID to update.
    #[serde(rename = "entry_id")]
    pub entry_id: i64,
    /// New weight value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// Optional timestamp override (ISO 8601).
    #[serde(rename = "logged_at", skip_serializing_if = "Option::is_none")]
    pub logged_at: Option<String>,
}

pub struct UpdateWeightEntry {
    clock: Clock,
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl UpdateWeightEntry {
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
impl Operation for UpdateWeightEntry {
    fn name(&self) -> &str {
        "update_weight_entry"
    }

    fn description(&self) -> &str {
        "Update a weight entry's value and/or timestamp. Errors if the entry does not exist."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(UpdateWeightEntryRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: UpdateWeightEntryRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path).await?
        } else {
            Connection::open().await?
        };

        #[cfg(not(test))]
        let conn = Connection::open().await?;

        // Verify entry exists
        {
            let mut stmt = conn
                .prepare("SELECT id FROM weight_entries WHERE id = ?")
                .await
                .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
            let mut rows = stmt
                .query((req.entry_id,))
                .await
                .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;
            if rows
                .next()
                .await
                .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?
                .is_none()
            {
                return Err(ErrorData::not_found());
            }
        }

        // Validate value if provided
        if let Some(v) = req.value
            && v <= 0.0
        {
            return Err(ErrorData::validation("value", "must be greater than zero"));
        }

        conn.execute("BEGIN", ())
            .await
            .map_err(|e| ErrorData::storage_failure(format!("transaction begin failed: {e}")))?;

        let result = (async {
            // Update value if provided
            if let Some(value) = req.value {
                conn.execute(
                    "UPDATE weight_entries SET value = ? WHERE id = ?",
                    (value, req.entry_id),
                )
                .await
                .map_err(|e| ErrorData::storage_failure(format!("update failed: {e}")))?;
            }

            // Update logged_at/logged_date if provided
            if let Some(ref ts) = req.logged_at {
                let (logged_at_str, logged_date_str) = parse_logged_at(ts, &self.clock)?;
                conn.execute(
                    "UPDATE weight_entries SET logged_at = ?, logged_date = ? WHERE id = ?",
                    (logged_at_str, logged_date_str, req.entry_id),
                )
                .await
                .map_err(|e| ErrorData::storage_failure(format!("update failed: {e}")))?;
            }

            build_weight_summary(&conn, req.entry_id).await
        })
        .await;

        match result {
            Ok(summary) => {
                conn.execute("COMMIT", ())
                    .await
                    .map_err(|e| ErrorData::storage_failure(format!("commit failed: {e}")))?;
                Ok(serde_json::to_value(summary).map_err(|e| {
                    ErrorData::storage_failure(format!("serialization failed: {e}"))
                })?)
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", ()).await;
                Err(e)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// DeleteWeightEntry Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct DeleteWeightEntryRequest {
    /// The weight entry ID to delete.
    #[serde(rename = "entry_id")]
    pub entry_id: i64,
}

pub struct DeleteWeightEntry {
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl Default for DeleteWeightEntry {
    fn default() -> Self {
        Self::new()
    }
}

impl DeleteWeightEntry {
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
impl Operation for DeleteWeightEntry {
    fn name(&self) -> &str {
        "delete_weight_entry"
    }

    fn description(&self) -> &str {
        "Delete a weight entry. Hard delete with no undo path. Errors if the entry does not exist."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(DeleteWeightEntryRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: DeleteWeightEntryRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path).await?
        } else {
            Connection::open().await?
        };

        #[cfg(not(test))]
        let conn = Connection::open().await?;

        // Verify entry exists
        {
            let mut stmt = conn
                .prepare("SELECT id FROM weight_entries WHERE id = ?")
                .await
                .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
            let mut rows = stmt
                .query((req.entry_id,))
                .await
                .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;
            if rows
                .next()
                .await
                .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?
                .is_none()
            {
                return Err(ErrorData::not_found());
            }
        }

        // Hard delete
        conn.execute("BEGIN", ())
            .await
            .map_err(|e| ErrorData::storage_failure(format!("transaction begin failed: {e}")))?;

        let result = (async {
            conn.execute("DELETE FROM weight_entries WHERE id = ?", (req.entry_id,))
                .await
                .map_err(|e| ErrorData::storage_failure(format!("delete failed: {e}")))?;
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
                    "entry_id": req.entry_id,
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
// GetWeightToday Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct GetWeightTodayRequest {}

pub struct GetWeightToday {
    clock: Clock,
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl GetWeightToday {
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
impl Operation for GetWeightToday {
    fn name(&self) -> &str {
        "get_weight_today"
    }

    fn description(&self) -> &str {
        "Get all weight entries for today (based on configured timezone)."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(GetWeightTodayRequest)).ok()
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

        let today_str = Clock::format_date(self.clock.today());

        let sql = "SELECT id, logged_at, logged_date, value FROM weight_entries WHERE logged_date = ? ORDER BY logged_at DESC";
        let mut stmt = conn
            .prepare(sql)
            .await
            .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
        let mut rows = stmt
            .query((&today_str[..],))
            .await
            .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

        let mut summaries = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
        {
            summaries.push(weight_entry_summary_from_row(&row)?);
        }

        Ok(serde_json::to_value(summaries)
            .map_err(|e| ErrorData::storage_failure(format!("serialization failed: {e}")))?)
    }
}

// ---------------------------------------------------------------------------
// GetWeightByDate Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct GetWeightByDateRequest {
    /// Date in YYYY-MM-DD format.
    pub date: String,
}

pub struct GetWeightByDate {
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl Default for GetWeightByDate {
    fn default() -> Self {
        Self::new()
    }
}

impl GetWeightByDate {
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
impl Operation for GetWeightByDate {
    fn name(&self) -> &str {
        "get_weight_by_date"
    }

    fn description(&self) -> &str {
        "Get all weight entries for a specific date (YYYY-MM-DD format)."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(GetWeightByDateRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: GetWeightByDateRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path).await?
        } else {
            Connection::open().await?
        };

        #[cfg(not(test))]
        let conn = Connection::open().await?;

        let sql = "SELECT id, logged_at, logged_date, value FROM weight_entries WHERE logged_date = ? ORDER BY logged_at DESC";
        let mut stmt = conn
            .prepare(sql)
            .await
            .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
        let mut rows = stmt
            .query((&req.date[..],))
            .await
            .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

        let mut summaries = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
        {
            summaries.push(weight_entry_summary_from_row(&row)?);
        }

        Ok(serde_json::to_value(summaries)
            .map_err(|e| ErrorData::storage_failure(format!("serialization failed: {e}")))?)
    }
}

// ---------------------------------------------------------------------------
// GetWeightByDateRange Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct GetWeightByDateRangeRequest {
    #[serde(rename = "start_date")]
    pub start_date: String,
    #[serde(rename = "end_date")]
    pub end_date: String,
}

pub struct GetWeightByDateRange {
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl Default for GetWeightByDateRange {
    fn default() -> Self {
        Self::new()
    }
}

impl GetWeightByDateRange {
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
impl Operation for GetWeightByDateRange {
    fn name(&self) -> &str {
        "get_weight_by_date_range"
    }

    fn description(&self) -> &str {
        "Get all weight entries within a date range (inclusive). Both dates in YYYY-MM-DD format. Covers get_weight_today and get_weight_by_date use cases by passing the same date for both bounds."
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(GetWeightByDateRangeRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: GetWeightByDateRangeRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path).await?
        } else {
            Connection::open().await?
        };

        #[cfg(not(test))]
        let conn = Connection::open().await?;

        let sql = r#"
            SELECT id, logged_at, logged_date, value FROM weight_entries
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
            summaries.push(weight_entry_summary_from_row(&row)?);
        }

        Ok(serde_json::to_value(summaries)
            .map_err(|e| ErrorData::storage_failure(format!("serialization failed: {e}")))?)
    }
}

// ---------------------------------------------------------------------------
// GetWeightTrend Operation (MCP-only — backs the weight-trend widget)
// ---------------------------------------------------------------------------

/// Number of most-recent weight entries returned by `get_weight_trend`.
const WEIGHT_TREND_ENTRY_LIMIT: i64 = 30;

/// Days back from the latest entry's date that the week-over-week delta's
/// baseline must be dated on or before (falls back to the earliest entry when
/// no such entry exists).
const WEIGHT_TREND_BASELINE_DAYS: u64 = 7;

#[derive(Debug, Deserialize, JsonSchema)]
struct GetWeightTrendRequest {}

/// One fetched weight sample, in the shape the pure trend computation needs.
#[derive(Debug, Clone, Copy)]
struct TrendSample {
    logged_at: DateTime<Utc>,
    logged_date: NaiveDate,
    value: f64,
}

/// A single point in the response's `entries` array (chronological order).
#[derive(Debug, Clone, serde::Serialize)]
struct TrendEntryPoint {
    #[serde(rename = "logged_date")]
    logged_date: String,
    value: f64,
}

/// Signed week-over-week delta plus its movement relative to the target
/// weight. All three fields are null when fewer than two entries exist (no
/// baseline to compare against).
#[derive(Debug, Clone, serde::Serialize)]
struct WeightTrendDelta {
    value: Option<f64>,
    #[serde(rename = "reference_date")]
    reference_date: Option<String>,
    /// `"toward_target"` / `"away_from_target"` / `"neutral"`. Deliberately
    /// not named "direction" — that term is reserved for the nutrient
    /// `Goal::Direction` enum (CONTEXT.md); weight progress carries no
    /// Direction, it is read directly off the comparison to the target.
    movement: Option<&'static str>,
}

/// Full `get_weight_trend` response shape. Nulls are explicit so the widget
/// has a stable contract to branch on (absent target, absent delta).
#[derive(Debug, Clone, serde::Serialize)]
struct WeightTrendResponse {
    entries: Vec<TrendEntryPoint>,
    delta: WeightTrendDelta,
    #[serde(rename = "target_weight")]
    target_weight: Option<f64>,
}

/// Compute the week-over-week delta and its movement relative to the target
/// weight, from fetched samples plus the active goal's target weight (if
/// any). Pure and DB-free so the rules are unit-testable in isolation:
///
/// - `current` is the entry with the max `logged_at`;
/// - the baseline is the entry with the max `logged_at` among those dated on
///   or before `current.logged_date - 7 days`, falling back to the earliest
///   entry when none qualifies;
/// - with fewer than two entries there is no baseline, so all delta fields
///   are null;
/// - `movement` is neutral when there is no target, when current is at the
///   target (same 1e-9 tolerance as `goal::weight_progress`), or when the
///   delta is zero; otherwise toward/away by whether the delta's sign matches
///   the sign of `target - current`. Works symmetrically for loss and gain
///   goals.
fn compute_weight_trend(samples: &[TrendSample], target_weight: Option<f64>) -> WeightTrendDelta {
    if samples.len() < 2 {
        return WeightTrendDelta {
            value: None,
            reference_date: None,
            movement: None,
        };
    }

    // `max_by`/`min_by` resolve timestamp ties deterministically (last / first
    // occurrence), so with >= 2 samples the baseline never pairs an entry
    // with itself.
    let current = samples
        .iter()
        .max_by(|a, b| a.logged_at.cmp(&b.logged_at))
        .unwrap();
    let cutoff = current.logged_date - chrono::Days::new(WEIGHT_TREND_BASELINE_DAYS);

    let baseline = samples
        .iter()
        .filter(|s| s.logged_date <= cutoff)
        .max_by(|a, b| a.logged_at.cmp(&b.logged_at))
        .unwrap_or_else(|| {
            samples
                .iter()
                .min_by(|a, b| a.logged_at.cmp(&b.logged_at))
                .unwrap()
        });

    let value = current.value - baseline.value;

    let movement = match target_weight {
        Some(target) if (current.value - target).abs() >= 1e-9 => {
            if value == 0.0 {
                "neutral"
            } else if (value > 0.0) == (target - current.value > 0.0) {
                "toward_target"
            } else {
                "away_from_target"
            }
        }
        _ => "neutral",
    };

    WeightTrendDelta {
        value: Some(value),
        reference_date: Some(Clock::format_date(baseline.logged_date)),
        movement: Some(movement),
    }
}

/// Fetch the active goal's `target_weight` as-of a given date (the most
/// recent goal row whose `effective_from <= as_of_date`), mirroring the query
/// pattern in `goal`/`weekly`. `None` when no goal row exists or the active
/// goal has no target weight set.
async fn fetch_active_target_weight(
    conn: &Connection,
    as_of_date: &str,
) -> Result<Option<f64>, ErrorData> {
    let sql = r#"
        SELECT target_weight
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
        Some(row) => Ok(row.get_value(0).ok().and_then(|v| match v {
            turso::Value::Real(r) => Some(r),
            _ => None,
        })),
        None => Ok(None),
    }
}

pub struct GetWeightTrend {
    clock: Clock,
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl GetWeightTrend {
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
impl Operation for GetWeightTrend {
    fn name(&self) -> &str {
        "get_weight_trend"
    }

    fn description(&self) -> &str {
        "Get the most recent weight entries (up to 30, oldest first) plus the signed week-over-week delta and its movement relative to the target weight. Backs the weight-trend widget."
    }

    fn surfaces(&self) -> Surfaces {
        Surfaces::MCP
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(GetWeightTrendRequest)).ok()
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

        // Most recent N entries, newest-first from SQL; reversed below so the
        // sparkline draws chronologically (oldest first).
        let sql = "SELECT id, logged_at, logged_date, value FROM weight_entries \
                   ORDER BY logged_at DESC LIMIT ?";
        let mut stmt = conn
            .prepare(sql)
            .await
            .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
        let mut rows = stmt
            .query((WEIGHT_TREND_ENTRY_LIMIT,))
            .await
            .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

        let mut samples: Vec<TrendSample> = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
        {
            let logged_at: String = row
                .get::<String>(1)
                .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
            let logged_date: String = row
                .get::<String>(2)
                .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
            let value: f64 = row
                .get::<f64>(3)
                .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?;
            samples.push(TrendSample {
                logged_at: logged_at.parse::<DateTime<Utc>>().map_err(|e| {
                    ErrorData::storage_failure(format!("invalid stored logged_at: {e}"))
                })?,
                logged_date: NaiveDate::parse_from_str(&logged_date, "%Y-%m-%d").map_err(|e| {
                    ErrorData::storage_failure(format!("invalid stored logged_date: {e}"))
                })?,
                value,
            });
        }
        samples.reverse();

        let today_str = Clock::format_date(self.clock.today());
        let target_weight = fetch_active_target_weight(&conn, &today_str).await?;

        let delta = compute_weight_trend(&samples, target_weight);

        let entries = samples
            .iter()
            .map(|s| TrendEntryPoint {
                logged_date: Clock::format_date(s.logged_date),
                value: s.value,
            })
            .collect();

        Ok(serde_json::to_value(WeightTrendResponse {
            entries,
            delta,
            target_weight,
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

    // ---- LogWeight tests (AC #2) ----

    #[serial_test::serial]
    #[tokio::test]
    async fn test_log_weight_default_timestamp() {
        let db = TempDb::new().await;
        let clock = clock();
        let op = LogWeight::new(clock).with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({ "value": 75.0 })))
            .await
            .unwrap();

        assert!(result["entry_id"].is_i64());
        assert!(result["logged_at"].is_string());
        assert!(result["logged_date"].is_string());
        assert_eq!(result["value"].as_f64().unwrap(), 75.0);

        // Verify logged_date matches today's date in UTC
        let today_str = Clock::format_date(clock.today());
        assert_eq!(result["logged_date"].as_str().unwrap(), today_str);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_log_weight_explicit_logged_at() {
        let db = TempDb::new().await;
        let clock = clock();
        let op = LogWeight::new(clock).with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "value": 75.0,
                "logged_at": "2025-01-15T10:30:00Z"
            })))
            .await
            .unwrap();

        assert_eq!(
            result["logged_at"].as_str().unwrap(),
            "2025-01-15T10:30:00Z"
        );
        assert_eq!(result["logged_date"].as_str().unwrap(), "2025-01-15");
        assert_eq!(result["value"].as_f64().unwrap(), 75.0);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_log_weight_rejects_non_positive_value() {
        let db = TempDb::new().await;
        let clock = clock();

        // Test zero value
        let op = LogWeight::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({ "value": 0.0 })))
            .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().category, ErrorCategory::Validation);

        // Test negative value
        let op = LogWeight::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({ "value": -5.0 })))
            .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().category, ErrorCategory::Validation);
    }

    // ---- UpdateWeightEntry tests (AC #3) ----

    async fn seed_entry(conn: &Connection, logged_at: &str, logged_date: &str, value: f64) -> i64 {
        let sql = "INSERT INTO weight_entries (logged_at, logged_date, value) VALUES (?, ?, ?) RETURNING id";
        let mut stmt = conn.prepare(sql).await.unwrap();
        let mut rows = stmt.query((logged_at, logged_date, value)).await.unwrap();
        match rows.next().await.unwrap() {
            Some(row) => match row.get_value(0).unwrap() {
                turso::Value::Integer(id) => id,
                _ => panic!("unexpected type"),
            },
            None => panic!("no row"),
        }
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_update_weight_entry_value_only() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let entry_id = seed_entry(&conn, "2025-01-15T08:00:00Z", "2025-01-15", 75.0).await;
        drop(conn);

        let clock = clock();
        let op = UpdateWeightEntry::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "entry_id": entry_id,
                "value": 80.0
            })))
            .await
            .unwrap();

        assert_eq!(result["value"].as_f64().unwrap(), 80.0);
        assert_eq!(
            result["logged_at"].as_str().unwrap(),
            "2025-01-15T08:00:00Z"
        );

        // Verify persistence via GetWeightByDate
        let get_op = GetWeightByDate::new().with_db_path(db.path.clone());
        let query_result = get_op
            .execute_json(Arc::new(serde_json::json!({ "date": "2025-01-15" })))
            .await
            .unwrap();
        let entries = query_result.as_array().unwrap();
        assert_eq!(entries[0]["value"].as_f64().unwrap(), 80.0);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_update_weight_entry_logged_at_only() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let entry_id = seed_entry(&conn, "2025-01-15T08:00:00Z", "2025-01-15", 75.0).await;
        drop(conn);

        let clock = clock();
        let op = UpdateWeightEntry::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "entry_id": entry_id,
                "logged_at": "2025-06-01T09:00:00Z"
            })))
            .await
            .unwrap();

        assert_eq!(
            result["logged_at"].as_str().unwrap(),
            "2025-06-01T09:00:00Z"
        );
        assert_eq!(result["logged_date"].as_str().unwrap(), "2025-06-01");
        assert_eq!(result["value"].as_f64().unwrap(), 75.0);
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_update_weight_entry_both_fields() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let entry_id = seed_entry(&conn, "2025-01-15T08:00:00Z", "2025-01-15", 75.0).await;
        drop(conn);

        let clock = clock();
        let op = UpdateWeightEntry::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "entry_id": entry_id,
                "value": 80.0,
                "logged_at": "2025-06-01T09:00:00Z"
            })))
            .await
            .unwrap();

        assert_eq!(result["value"].as_f64().unwrap(), 80.0);
        assert_eq!(
            result["logged_at"].as_str().unwrap(),
            "2025-06-01T09:00:00Z"
        );
        assert_eq!(result["logged_date"].as_str().unwrap(), "2025-06-01");
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_update_weight_entry_not_found() {
        let db = TempDb::new().await;
        let clock = clock();
        let op = UpdateWeightEntry::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "entry_id": 99999,
                "value": 80.0
            })))
            .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().category, ErrorCategory::NotFound);
    }

    // ---- DeleteWeightEntry tests (AC #4) ----

    #[serial_test::serial]
    #[tokio::test]
    async fn test_delete_weight_entry_success() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        let entry_id = seed_entry(&conn, "2025-01-15T08:00:00Z", "2025-01-15", 75.0).await;
        drop(conn);

        let op = DeleteWeightEntry::new().with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({ "entry_id": entry_id })))
            .await
            .unwrap();

        assert!(result["deleted"].as_bool().unwrap());
        assert_eq!(result["entry_id"].as_i64().unwrap(), entry_id);

        // Verify entry is gone
        let get_op = GetWeightByDate::new().with_db_path(db.path.clone());
        let query_result = get_op
            .execute_json(Arc::new(serde_json::json!({ "date": "2025-01-15" })))
            .await
            .unwrap();
        assert!(query_result.as_array().unwrap().is_empty());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_delete_weight_entry_not_found() {
        let db = TempDb::new().await;
        let op = DeleteWeightEntry::new().with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({ "entry_id": 99999 })))
            .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().category, ErrorCategory::NotFound);
    }

    // ---- GetWeightToday tests (AC #5) ----

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_today_empty() {
        let db = TempDb::new().await;
        let clock = clock();
        let op = GetWeightToday::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();
        assert!(result.is_array());
        assert!(result.as_array().unwrap().is_empty());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_today_populated() {
        let db = TempDb::new().await;
        let clock = clock();
        let today_str = Clock::format_date(clock.today());

        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_entry(&conn, "2025-01-15T08:00:00Z", &today_str, 75.0).await;
        seed_entry(&conn, "2025-01-15T12:00:00Z", &today_str, 76.0).await;
        drop(conn);

        let op = GetWeightToday::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();

        let entries = result.as_array().unwrap();
        assert_eq!(entries.len(), 2);
        for entry in entries {
            assert!(entry["id"].is_i64());
            assert!(entry["logged_at"].is_string());
            assert!(entry["logged_date"].is_string());
            assert!(entry["value"].is_number());
        }
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_today_ordering_desc() {
        let db = TempDb::new().await;
        let clock = clock();
        let today_str = Clock::format_date(clock.today());

        // Insert in reverse chronological order
        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_entry(&conn, "2025-01-15T18:00:00Z", &today_str, 74.0).await;
        seed_entry(&conn, "2025-01-15T12:00:00Z", &today_str, 75.0).await;
        seed_entry(&conn, "2025-01-15T08:00:00Z", &today_str, 76.0).await;
        drop(conn);

        let op = GetWeightToday::new(clock).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();

        let entries = result.as_array().unwrap();
        assert_eq!(entries.len(), 3);
        // Should be newest-first (DESC by logged_at)
        assert_eq!(
            entries[0]["logged_at"].as_str().unwrap(),
            "2025-01-15T18:00:00Z"
        );
        assert_eq!(
            entries[1]["logged_at"].as_str().unwrap(),
            "2025-01-15T12:00:00Z"
        );
        assert_eq!(
            entries[2]["logged_at"].as_str().unwrap(),
            "2025-01-15T08:00:00Z"
        );
    }

    // ---- GetWeightByDate tests (AC #5) ----

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_by_date_empty() {
        let db = TempDb::new().await;
        let op = GetWeightByDate::new().with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({ "date": "2025-01-15" })))
            .await
            .unwrap();
        assert!(result.is_array());
        assert!(result.as_array().unwrap().is_empty());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_by_date_populated() {
        let db = TempDb::new().await;
        let test_date = "2025-01-15";

        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_entry(&conn, "2025-01-15T08:00:00Z", test_date, 75.0).await;
        seed_entry(&conn, "2025-01-15T12:00:00Z", test_date, 76.0).await;
        drop(conn);

        let op = GetWeightByDate::new().with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({ "date": test_date })))
            .await
            .unwrap();

        let entries = result.as_array().unwrap();
        assert_eq!(entries.len(), 2);
        for entry in entries {
            assert!(entry["id"].is_i64());
            assert!(entry["logged_at"].is_string());
            assert!(entry["logged_date"].is_string());
            assert!(entry["value"].is_number());
        }
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_by_date_ordering_desc() {
        let db = TempDb::new().await;
        let test_date = "2025-01-15";

        // Insert in reverse chronological order
        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_entry(&conn, "2025-01-15T18:00:00Z", test_date, 74.0).await;
        seed_entry(&conn, "2025-01-15T12:00:00Z", test_date, 75.0).await;
        seed_entry(&conn, "2025-01-15T08:00:00Z", test_date, 76.0).await;
        drop(conn);

        let op = GetWeightByDate::new().with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({ "date": test_date })))
            .await
            .unwrap();

        let entries = result.as_array().unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[0]["logged_at"].as_str().unwrap(),
            "2025-01-15T18:00:00Z"
        );
        assert_eq!(
            entries[1]["logged_at"].as_str().unwrap(),
            "2025-01-15T12:00:00Z"
        );
        assert_eq!(
            entries[2]["logged_at"].as_str().unwrap(),
            "2025-01-15T08:00:00Z"
        );
    }

    // ---- GetWeightByDateRange tests (AC #5) ----

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_by_date_range_empty() {
        let db = TempDb::new().await;
        let op = GetWeightByDateRange::new().with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "start_date": "2025-01-01",
                "end_date": "2025-01-31"
            })))
            .await
            .unwrap();
        assert!(result.is_array());
        assert!(result.as_array().unwrap().is_empty());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_by_date_range_populated() {
        let db = TempDb::new().await;

        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_entry(&conn, "2025-01-10T08:00:00Z", "2025-01-10", 75.0).await;
        seed_entry(&conn, "2025-01-20T12:00:00Z", "2025-01-20", 76.0).await;
        drop(conn);

        let op = GetWeightByDateRange::new().with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "start_date": "2025-01-01",
                "end_date": "2025-01-31"
            })))
            .await
            .unwrap();

        let entries = result.as_array().unwrap();
        assert_eq!(entries.len(), 2);
        for entry in entries {
            assert!(entry["id"].is_i64());
            assert!(entry["logged_at"].is_string());
            assert!(entry["logged_date"].is_string());
            assert!(entry["value"].is_number());
        }
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_by_date_range_ordering_desc() {
        let db = TempDb::new().await;

        // Insert in reverse chronological order across dates
        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_entry(&conn, "2025-01-15T18:00:00Z", "2025-01-15", 74.0).await;
        seed_entry(&conn, "2025-01-10T12:00:00Z", "2025-01-10", 75.0).await;
        seed_entry(&conn, "2025-01-05T08:00:00Z", "2025-01-05", 76.0).await;
        drop(conn);

        let op = GetWeightByDateRange::new().with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({
                "start_date": "2025-01-01",
                "end_date": "2025-01-31"
            })))
            .await
            .unwrap();

        let entries = result.as_array().unwrap();
        assert_eq!(entries.len(), 3);
        // Should be newest-first (DESC by logged_at)
        assert_eq!(
            entries[0]["logged_at"].as_str().unwrap(),
            "2025-01-15T18:00:00Z"
        );
        assert_eq!(
            entries[1]["logged_at"].as_str().unwrap(),
            "2025-01-10T12:00:00Z"
        );
        assert_eq!(
            entries[2]["logged_at"].as_str().unwrap(),
            "2025-01-05T08:00:00Z"
        );
    }

    // ---- GetWeightTrend tests (AC #2, #3, #5) ----

    /// Build a [`TrendSample`] from a YYYY-MM-DD date, HH:MM:SS time, and value.
    fn trend_sample(date: &str, time: &str, value: f64) -> TrendSample {
        TrendSample {
            logged_at: format!("{date}T{time}Z").parse().unwrap(),
            logged_date: date.parse().unwrap(),
            value,
        }
    }

    // ---- Pure delta/movement unit tests (no I/O) ----

    #[test]
    fn test_compute_weight_trend_zero_entries() {
        let d = compute_weight_trend(&[], Some(75.0));
        assert!(d.value.is_none());
        assert!(d.reference_date.is_none());
        assert!(d.movement.is_none());
    }

    #[test]
    fn test_compute_weight_trend_one_entry() {
        let samples = [trend_sample("2025-01-15", "10:00:00", 76.0)];
        let d = compute_weight_trend(&samples, Some(75.0));
        assert!(d.value.is_none());
        assert!(d.reference_date.is_none());
        assert!(d.movement.is_none());
    }

    #[test]
    fn test_compute_weight_trend_fallback_to_earliest_within_seven_days() {
        // Both entries within 7 days of each other: no entry qualifies as a
        // baseline (dated <= current - 7d), so the earliest is used.
        let samples = [
            trend_sample("2025-01-15", "10:00:00", 76.0),
            trend_sample("2025-01-18", "09:00:00", 75.5),
        ];
        let d = compute_weight_trend(&samples, Some(75.0));
        assert_eq!(d.value, Some(-0.5));
        assert_eq!(d.reference_date.as_deref(), Some("2025-01-15"));
        // Losing weight toward a lower target: toward_target.
        assert_eq!(d.movement, Some("toward_target"));
    }

    #[test]
    fn test_compute_weight_trend_exact_seven_day_baseline() {
        // Baseline dated exactly 7 days before current qualifies.
        let samples = [
            trend_sample("2025-01-11", "08:00:00", 76.0),
            trend_sample("2025-01-18", "09:00:00", 75.5),
        ];
        let d = compute_weight_trend(&samples, Some(75.0));
        assert_eq!(d.value, Some(-0.5));
        assert_eq!(d.reference_date.as_deref(), Some("2025-01-11"));
    }

    #[test]
    fn test_compute_weight_trend_nearest_baseline_at_or_before_cutoff() {
        // 2025-01-12 is after the cutoff (2025-01-11) and must be skipped;
        // the newest entry at or before the cutoff (01-11 12:00) wins over
        // the older 01-10 entry.
        let samples = [
            trend_sample("2025-01-10", "08:00:00", 77.0),
            trend_sample("2025-01-11", "12:00:00", 76.0),
            trend_sample("2025-01-12", "08:00:00", 76.2),
            trend_sample("2025-01-18", "09:00:00", 75.5),
        ];
        let d = compute_weight_trend(&samples, Some(75.0));
        assert_eq!(d.value, Some(-0.5));
        assert_eq!(d.reference_date.as_deref(), Some("2025-01-11"));
    }

    #[test]
    fn test_compute_weight_trend_loss_goal_toward_and_away() {
        // Loss goal (target below current): falling = toward, rising = away.
        let toward = [
            trend_sample("2025-01-11", "08:00:00", 76.5),
            trend_sample("2025-01-18", "09:00:00", 76.0),
        ];
        assert_eq!(
            compute_weight_trend(&toward, Some(75.0)).movement,
            Some("toward_target")
        );

        let away = [
            trend_sample("2025-01-11", "08:00:00", 75.5),
            trend_sample("2025-01-18", "09:00:00", 76.0),
        ];
        assert_eq!(
            compute_weight_trend(&away, Some(75.0)).movement,
            Some("away_from_target")
        );
    }

    #[test]
    fn test_compute_weight_trend_gain_goal_toward_and_away() {
        // Gain goal (target above current): rising = toward, falling = away.
        let toward = [
            trend_sample("2025-01-11", "08:00:00", 77.5),
            trend_sample("2025-01-18", "09:00:00", 78.0),
        ];
        assert_eq!(
            compute_weight_trend(&toward, Some(80.0)).movement,
            Some("toward_target")
        );

        let away = [
            trend_sample("2025-01-11", "08:00:00", 78.5),
            trend_sample("2025-01-18", "09:00:00", 78.0),
        ];
        assert_eq!(
            compute_weight_trend(&away, Some(80.0)).movement,
            Some("away_from_target")
        );
    }

    #[test]
    fn test_compute_weight_trend_at_target_is_neutral() {
        // Current equals the target (within tolerance): neutral regardless of
        // a nonzero delta.
        let samples = [
            trend_sample("2025-01-11", "08:00:00", 76.5),
            trend_sample("2025-01-18", "09:00:00", 76.0),
        ];
        let d = compute_weight_trend(&samples, Some(76.0));
        assert_eq!(d.value, Some(-0.5));
        assert_eq!(d.movement, Some("neutral"));
    }

    #[test]
    fn test_compute_weight_trend_no_target_is_neutral() {
        let samples = [
            trend_sample("2025-01-11", "08:00:00", 76.5),
            trend_sample("2025-01-18", "09:00:00", 76.0),
        ];
        let d = compute_weight_trend(&samples, None);
        assert_eq!(d.value, Some(-0.5));
        assert_eq!(d.movement, Some("neutral"));
    }

    #[test]
    fn test_compute_weight_trend_zero_delta_is_neutral() {
        // Unchanged weight with a reachable target: neutral, not toward/away.
        let samples = [
            trend_sample("2025-01-11", "08:00:00", 76.0),
            trend_sample("2025-01-18", "09:00:00", 76.0),
        ];
        let d = compute_weight_trend(&samples, Some(75.0));
        assert_eq!(d.value, Some(0.0));
        assert_eq!(d.movement, Some("neutral"));
    }

    #[test]
    fn test_compute_weight_trend_thirty_samples() {
        // 30 daily samples: the cap-sized input computes against the newest
        // entry at or before the 7-day cutoff, not the overall earliest.
        let samples: Vec<TrendSample> = (0..30)
            .map(|i| {
                let date = chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
                    + chrono::Days::new(i as u64);
                trend_sample(&Clock::format_date(date), "08:00:00", 70.0 + i as f64 * 0.1)
            })
            .collect();
        let d = compute_weight_trend(&samples, Some(70.0));
        // current = Jan 30 (value 72.9); cutoff = Jan 23; baseline = Jan 23
        // (value 72.2).
        assert!((d.value.unwrap() - 0.7).abs() < 1e-9);
        assert_eq!(d.reference_date.as_deref(), Some("2025-01-23"));
        assert_eq!(d.movement, Some("away_from_target"));
    }

    // ---- GetWeightTrend integration tests (execute_json vs TempDb) ----

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_trend_empty_db() {
        let db = TempDb::new().await;
        let op = GetWeightTrend::new(clock()).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();

        assert!(result["entries"].as_array().unwrap().is_empty());
        assert!(result["delta"]["value"].is_null());
        assert!(result["delta"]["reference_date"].is_null());
        assert!(result["delta"]["movement"].is_null());
        assert!(result["target_weight"].is_null());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_trend_end_to_end_shape_with_target() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        seed_entry(&conn, "2025-01-11T08:00:00Z", "2025-01-11", 76.0).await;
        seed_entry(&conn, "2025-01-18T09:00:00Z", "2025-01-18", 75.5).await;
        conn.execute(
            "INSERT INTO goals (effective_from, target_weight) VALUES ('2025-01-01', 75.0)",
            (),
        )
        .await
        .unwrap();
        drop(conn);

        let op = GetWeightTrend::new(clock()).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();

        // Entries chronological (oldest first), projected to date+value only.
        let entries = result["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["logged_date"].as_str(), Some("2025-01-11"));
        assert_eq!(entries[0]["value"].as_f64(), Some(76.0));
        assert_eq!(entries[1]["logged_date"].as_str(), Some("2025-01-18"));
        assert_eq!(entries[1]["value"].as_f64(), Some(75.5));
        assert!(entries[0].get("id").is_none());
        assert!(entries[0].get("logged_at").is_none());

        // Delta: 7-day-exact baseline, falling toward the lower target.
        assert_eq!(result["delta"]["value"].as_f64(), Some(-0.5));
        assert_eq!(
            result["delta"]["reference_date"].as_str(),
            Some("2025-01-11")
        );
        assert_eq!(result["delta"]["movement"].as_str(), Some("toward_target"));

        // Target joined from the active goal.
        assert_eq!(result["target_weight"].as_f64(), Some(75.0));
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_trend_multiple_entries_same_date() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        // Two weigh-ins on the same date: both surface, and the later
        // timestamp is the current reading.
        seed_entry(&conn, "2025-01-18T08:00:00Z", "2025-01-18", 75.2).await;
        seed_entry(&conn, "2025-01-18T18:00:00Z", "2025-01-18", 75.0).await;
        drop(conn);

        let op = GetWeightTrend::new(clock()).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();

        let entries = result["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1]["value"].as_f64(), Some(75.0));
        // Fallback baseline is the earlier same-day entry.
        let delta_value = result["delta"]["value"].as_f64().unwrap();
        assert!((delta_value - (-0.2)).abs() < 1e-9);
        assert_eq!(
            result["delta"]["reference_date"].as_str(),
            Some("2025-01-18")
        );
        // No goal row: neutral movement, null target.
        assert_eq!(result["delta"]["movement"].as_str(), Some("neutral"));
        assert!(result["target_weight"].is_null());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_trend_caps_at_thirty_entries() {
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();
        // 35 daily entries; only the 30 most recent may come back.
        for i in 0..35u32 {
            let date =
                chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap() + chrono::Days::new(i as u64);
            let ts = format!("{}T08:00:00Z", Clock::format_date(date));
            seed_entry(&conn, &ts, &Clock::format_date(date), 70.0 + i as f64 * 0.1).await;
        }
        drop(conn);

        let op = GetWeightTrend::new(clock()).with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();

        let entries = result["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 30);
        // Oldest surviving entry is the 6th inserted (Jan 6); the five
        // oldest (Jan 1-5) were cut by the cap. Newest is Jan 1 + 34d.
        assert_eq!(entries[0]["logged_date"].as_str(), Some("2025-01-06"));
        assert_eq!(entries[29]["logged_date"].as_str(), Some("2025-02-04"));
    }
}

//! Weight entry operations — log, update, delete, and query by date.
//!
//! Implements `log_weight`, `update_weight_entry`, `delete_weight_entry`,
//! `get_weight_today`, `get_weight_by_date`, and `get_weight_by_date_range`
//! per doc-5 §5, §13.
//!
//! Weight entries are simple: no FK relationships, no snapshotting, no computed
//! totals — just raw value storage with temporal handling. All deletes are
//! hard deletes with no undo path.

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
        Some(row) => Ok(WeightEntrySummary {
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
        }),
        None => Err(ErrorData::not_found()),
    }
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
            .query((logged_at_str.clone(), logged_date_str.clone(), req.value))
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
        if let Some(v) = req.value {
            if v <= 0.0 {
                return Err(ErrorData::validation("value", "must be greater than zero"));
            }
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
                let dt: DateTime<Utc> = ts.parse().map_err(|_| {
                    ErrorData::validation("logged_at", format!("invalid datetime format: {}", ts))
                })?;
                let logged_at_str = format!("{}", dt.format("%Y-%m-%dT%H:%M:%SZ"));
                let logged_date_str = Clock::format_date(self.clock.logged_date(&dt));
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
struct GetWeightTodayRequest;

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
}

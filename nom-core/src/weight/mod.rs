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
            summaries.push(WeightEntrySummary {
                id: row.get::<i64>(0).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
                logged_at: row.get::<String>(1).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
                logged_date: row.get::<String>(2).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
                value: row.get::<f64>(3).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
            });
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
            summaries.push(WeightEntrySummary {
                id: row.get::<i64>(0).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
                logged_at: row.get::<String>(1).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
                logged_date: row.get::<String>(2).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
                value: row.get::<f64>(3).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
            });
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
            summaries.push(WeightEntrySummary {
                id: row.get::<i64>(0).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
                logged_at: row.get::<String>(1).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
                logged_date: row.get::<String>(2).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
                value: row.get::<f64>(3).map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?,
            });
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
    use crate::storage::test::TempDb;

    #[serial_test::serial]
    #[tokio::test]
    async fn test_get_weight_by_date_returns_all_entries() {
        // Regression test: seed 3 weight entries for the same date and verify
        // GetWeightByDate returns all three, guarding against silent row drops.
        let db = TempDb::new().await;
        let conn = Connection::open_at(&db.path).await.unwrap();

        let test_date = "2025-01-15";
        conn.execute(
            "INSERT INTO weight_entries (logged_at, logged_date, value) VALUES (?, ?, ?)",
            ("2025-01-15T08:00:00Z", test_date, 180.5),
        ).await.unwrap();
        conn.execute(
            "INSERT INTO weight_entries (logged_at, logged_date, value) VALUES (?, ?, ?)",
            ("2025-01-15T12:00:00Z", test_date, 179.0),
        ).await.unwrap();
        conn.execute(
            "INSERT INTO weight_entries (logged_at, logged_date, value) VALUES (?, ?, ?)",
            ("2025-01-15T18:00:00Z", test_date, 178.5),
        ).await.unwrap();
        drop(conn);

        let op = GetWeightByDate::new().with_db_path(db.path.clone());
        let result = op
            .execute_json(Arc::new(serde_json::json!({ "date": test_date })))
            .await
            .unwrap();

        let entries = result.as_array().expect("should return an array");
        assert_eq!(entries.len(), 3, "all seeded entries must be returned");

        // Verify ordered by logged_at DESC (latest first)
        assert_eq!(entries[0]["value"].as_f64().unwrap(), 178.5);
        assert_eq!(entries[1]["value"].as_f64().unwrap(), 179.0);
        assert_eq!(entries[2]["value"].as_f64().unwrap(), 180.5);

        // Verify all fields present on each entry
        for entry in entries {
            assert!(entry["id"].is_i64());
            assert!(entry["logged_at"].is_string());
            assert!(entry["logged_date"].is_string());
            assert!(entry["value"].is_number());
        }
    }
}

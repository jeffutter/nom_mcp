//! Widget Display — MCP-only operations for enabling/disabling widget display.
//!
//! Backed by the `settings` table (`widget_display_enabled` BOOLEAN).
//! v1 is plumbing only — no tool or Resource output branches on it yet.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;

use crate::error::ErrorData;
use crate::operation::{Operation, Surfaces};
use crate::storage::Connection;

// ---------------------------------------------------------------------------
// GetWidgetDisplay Operation
// ---------------------------------------------------------------------------

pub struct GetWidgetDisplay {
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl Default for GetWidgetDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl GetWidgetDisplay {
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
impl Operation for GetWidgetDisplay {
    fn name(&self) -> &str {
        "get_widget_display"
    }

    fn description(&self) -> &str {
        "Get the current widget display setting. Returns {enabled: bool}."
    }

    fn surfaces(&self) -> Surfaces {
        Surfaces::MCP
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

        let sql = "SELECT widget_display_enabled FROM settings LIMIT 1";
        let mut stmt = conn
            .prepare(sql)
            .await
            .map_err(|e| ErrorData::storage_failure(format!("prepare failed: {e}")))?;
        let mut rows = stmt
            .query(())
            .await
            .map_err(|e| ErrorData::storage_failure(format!("query failed: {e}")))?;

        let enabled = match rows
            .next()
            .await
            .map_err(|e| ErrorData::storage_failure(format!("read error: {e}")))?
        {
            Some(row) => row
                .get_value(0)
                .map(|v| match v {
                    turso::Value::Integer(n) => n != 0,
                    turso::Value::Real(r) => r != 0.0,
                    _ => false,
                })
                .unwrap_or(false),
            None => false,
        };

        Ok(serde_json::json!({ "enabled": enabled }))
    }
}

// ---------------------------------------------------------------------------
// SetWidgetDisplay Operation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct SetWidgetDisplayRequest {
    /// Whether widget display should be enabled.
    enabled: bool,
}

pub struct SetWidgetDisplay {
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl Default for SetWidgetDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl SetWidgetDisplay {
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
impl Operation for SetWidgetDisplay {
    fn name(&self) -> &str {
        "set_widget_display"
    }

    fn description(&self) -> &str {
        "Enable or disable widget display. Takes {enabled: bool}."
    }

    fn surfaces(&self) -> Surfaces {
        Surfaces::MCP
    }

    fn input_schema(&self) -> Option<serde_json::Value> {
        serde_json::to_value(schemars::schema_for!(SetWidgetDisplayRequest)).ok()
    }

    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, ErrorData> {
        let req: SetWidgetDisplayRequest = serde_json::from_value((*args).clone())
            .map_err(|e| ErrorData::validation("request", format!("invalid request: {e}")))?;

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path).await?
        } else {
            Connection::open().await?
        };

        #[cfg(not(test))]
        let conn = Connection::open().await?;

        let enabled_int = if req.enabled { 1 } else { 0 };

        // UPDATE then INSERT-if-no-changes pattern for single-row table
        let sql_update = "UPDATE settings SET widget_display_enabled = ?";
        let changes = conn
            .execute(sql_update, (enabled_int,))
            .await
            .map_err(|e| ErrorData::storage_failure(format!("update failed: {e}")))?;

        if changes == 0 {
            // Row doesn't exist, insert
            let sql_insert = "INSERT INTO settings (widget_display_enabled) VALUES (?)";
            conn.execute(sql_insert, (enabled_int,))
                .await
                .map_err(|e| ErrorData::storage_failure(format!("insert failed: {e}")))?;
        }

        Ok(serde_json::json!({ "enabled": req.enabled }))
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
    async fn test_get_widget_display_default_false() {
        let db = TempDb::new().await;
        let op = GetWidgetDisplay::new().with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();

        assert!(!result["enabled"].as_bool().unwrap());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_set_widget_display_true() {
        let db = TempDb::new().await;
        let op = SetWidgetDisplay::new().with_db_path(db.path.clone());

        let result = op
            .execute_json(Arc::new(serde_json::json!({ "enabled": true })))
            .await
            .unwrap();

        assert!(result["enabled"].as_bool().unwrap());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_set_widget_display_then_get() {
        let db = TempDb::new().await;

        // Set to true
        let set_op = SetWidgetDisplay::new().with_db_path(db.path.clone());
        let set_result = set_op
            .execute_json(Arc::new(serde_json::json!({ "enabled": true })))
            .await
            .unwrap();
        assert!(set_result["enabled"].as_bool().unwrap());

        // Verify persisted via get
        let get_op = GetWidgetDisplay::new().with_db_path(db.path.clone());
        let get_result = get_op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();
        assert!(get_result["enabled"].as_bool().unwrap());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_set_widget_display_false() {
        let db = TempDb::new().await;

        // First set to true
        let set_op = SetWidgetDisplay::new().with_db_path(db.path.clone());
        set_op
            .execute_json(Arc::new(serde_json::json!({ "enabled": true })))
            .await
            .unwrap();

        // Then toggle back to false
        let set_op = SetWidgetDisplay::new().with_db_path(db.path.clone());
        let result = set_op
            .execute_json(Arc::new(serde_json::json!({ "enabled": false })))
            .await
            .unwrap();
        assert!(!result["enabled"].as_bool().unwrap());

        // Verify via get
        let get_op = GetWidgetDisplay::new().with_db_path(db.path.clone());
        let get_result = get_op
            .execute_json(Arc::new(serde_json::json!({})))
            .await
            .unwrap();
        assert!(!get_result["enabled"].as_bool().unwrap());
    }
}

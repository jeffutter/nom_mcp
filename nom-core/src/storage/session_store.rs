//! Persistent store for MCP streamable-HTTP sessions (rmcp `SessionStore`).
//!
//! Backed by a dedicated SQLite file (`mcp_sessions.db`, see
//! [`crate::config::session_db_path`]) rather than the domain database: this
//! store keeps one connection open for the server process's lifetime, and a
//! live turso connection holds the advisory write lock on its file — sharing
//! `nom.db` would make every operation's `Connection::open()` lock probe
//! report `local_db_locked`.
//!
//! With this store wired into `StreamableHttpServerConfig.session_store`,
//! rmcp persists each session's initialize params after a successful
//! handshake, deletes them on session close, and transparently restores
//! unknown session IDs after a restart (recreating the in-memory worker and
//! replaying the initialize handshake). Clients that keep their
//! `Mcp-Session-Id` across deploys therefore keep working instead of hitting
//! the spec-mandated 404 that aborts Claude iOS widget loads.

use std::path::Path;

use super::connection::StorageError;
use async_trait::async_trait;
use rmcp::model::InitializeRequestParams;
use rmcp::transport::streamable_http_server::session::store::{
    SessionState, SessionStore, SessionStoreError,
};
use tokio::sync::Mutex;
use turso::{Builder, Connection as TursoConnection};

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS mcp_sessions ( \
     session_id TEXT PRIMARY KEY, \
     initialize_params TEXT NOT NULL, \
     stored_at TEXT NOT NULL DEFAULT (datetime('now')) \
     )";

/// rmcp [`SessionStore`] implementation over a dedicated SQLite file.
pub struct McpSessionStore {
    conn: Mutex<TursoConnection>,
}

impl McpSessionStore {
    /// Open (or create) the session database at `path` and ensure the schema
    /// exists. The connection stays open for the lifetime of the store.
    pub async fn open_at(path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StorageError::Io(format!(
                    "failed to create directory {}: {e}",
                    parent.display()
                ))
            })?;
        }
        let db =
            Builder::new_local(path.to_str().ok_or_else(|| {
                StorageError::Io("session db path contains invalid UTF-8".into())
            })?)
            .build()
            .await
            .map_err(|e| StorageError::Database(format!("failed to build session db: {e}")))?;
        let conn = db
            .connect()
            .map_err(|e| StorageError::Database(format!("failed to connect session db: {e}")))?;
        conn.execute(SCHEMA, ())
            .await
            .map_err(|e| StorageError::Query(format!("failed to create session table: {e}")))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Checkpoint the WAL before releasing the connection (same invariant as
    /// [`crate::storage::connection::Connection::checkpoint`]). Call during
    /// graceful shutdown.
    pub async fn checkpoint(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().await;
        // The PRAGMA returns a row, so this must be a query (not an exec);
        // propagate errors and discard the rows.
        let _rows = conn
            .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
            .await
            .map_err(|e| {
                StorageError::Database(format!("session db wal checkpoint failed: {e}"))
            })?;
        conn.cacheflush()
            .map_err(|e| StorageError::Database(format!("session db cache flush failed: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl SessionStore for McpSessionStore {
    async fn load(&self, session_id: &str) -> Result<Option<SessionState>, SessionStoreError> {
        let mut stmt = self
            .conn
            .lock()
            .await
            .prepare("SELECT initialize_params FROM mcp_sessions WHERE session_id = ?1")
            .await
            .map_err(|e| {
                StorageError::Database(format!("failed to prepare session load statement: {e}"))
            })?;
        let mut rows = stmt
            .query((session_id,))
            .await
            .map_err(|e| StorageError::Query(format!("session load query failed: {e}")))?;
        let Some(row) = rows
            .next()
            .await
            .map_err(|e| StorageError::Query(format!("failed to read session row: {e}")))?
        else {
            return Ok(None);
        };
        let params = match row
            .get_value(0)
            .map_err(|e| StorageError::Query(format!("failed to get session value: {e}")))?
        {
            turso::Value::Text(t) => t,
            other => {
                return Err(
                    format!("unexpected value type for initialize_params: {other:?}").into(),
                );
            }
        };
        let initialize_params = serde_json::from_str::<InitializeRequestParams>(&params)
            .map_err(|e| format!("failed to deserialize stored initialize params: {e}"))?;
        Ok(Some(SessionState::new(initialize_params)))
    }

    async fn store(&self, session_id: &str, state: &SessionState) -> Result<(), SessionStoreError> {
        let params = serde_json::to_string(&state.initialize_params)
            .map_err(|e| format!("failed to serialize initialize params: {e}"))?;
        self.conn
            .lock()
            .await
            .execute(
                "INSERT OR REPLACE INTO mcp_sessions (session_id, initialize_params) \
                 VALUES (?1, ?2)",
                (session_id, params.as_str()),
            )
            .await
            .map_err(|e| StorageError::Query(format!("session store failed: {e}")))?;
        Ok(())
    }

    async fn delete(&self, session_id: &str) -> Result<(), SessionStoreError> {
        self.conn
            .lock()
            .await
            .execute(
                "DELETE FROM mcp_sessions WHERE session_id = ?1",
                (session_id,),
            )
            .await
            .map_err(|e| StorageError::Query(format!("session delete failed: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageError;
    use serial_test::serial;
    use tempfile::TempDir;

    /// Build an `InitializeRequestParams` via JSON — the struct is
    /// `#[non_exhaustive]`, so it can't be constructed directly outside rmcp.
    fn sample_init_params() -> InitializeRequestParams {
        serde_json::from_value(serde_json::json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "0.1.0" }
        }))
        .expect("valid initialize params JSON")
    }

    fn state(params: InitializeRequestParams) -> SessionState {
        SessionState::new(params)
    }

    #[tokio::test]
    async fn test_store_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let store = McpSessionStore::open_at(&dir.path().join("sessions.db"))
            .await
            .unwrap();
        let s = state(sample_init_params());

        store.store("sess-1", &s).await.unwrap();
        let loaded = store.load("sess-1").await.unwrap().expect("stored session");
        assert_eq!(loaded.initialize_params.client_info.name, "test-client");
        assert_eq!(
            loaded.initialize_params.protocol_version.as_str(),
            "2025-11-25"
        );
    }

    #[tokio::test]
    async fn test_load_missing_returns_none() {
        let dir = TempDir::new().unwrap();
        let store = McpSessionStore::open_at(&dir.path().join("sessions.db"))
            .await
            .unwrap();
        assert!(store.load("nope").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_removes_session() {
        let dir = TempDir::new().unwrap();
        let store = McpSessionStore::open_at(&dir.path().join("sessions.db"))
            .await
            .unwrap();
        let s = state(sample_init_params());
        store.store("sess-1", &s).await.unwrap();
        store.delete("sess-1").await.unwrap();
        assert!(store.load("sess-1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_store_replaces_existing() {
        let dir = TempDir::new().unwrap();
        let store = McpSessionStore::open_at(&dir.path().join("sessions.db"))
            .await
            .unwrap();
        let s = state(sample_init_params());
        store.store("sess-1", &s).await.unwrap();
        store.store("sess-1", &s).await.unwrap(); // INSERT OR REPLACE must not error
        assert!(store.load("sess-1").await.unwrap().is_some());
    }

    #[tokio::test]
    #[serial]
    async fn test_open_is_idempotent_and_persists_across_reopen() {
        // Two opens against the same file: DDL must be idempotent and data
        // must survive a reopen (the cross-restart guarantee).
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sessions.db");
        {
            let store = McpSessionStore::open_at(&path).await.unwrap();
            store
                .store("sess-x", &state(sample_init_params()))
                .await
                .unwrap();
            store.checkpoint().await.unwrap();
        }
        let store2 = McpSessionStore::open_at(&path).await.unwrap();
        let loaded = store2
            .load("sess-x")
            .await
            .unwrap()
            .expect("persisted session");
        assert_eq!(loaded.initialize_params.client_info.name, "test-client");
    }

    #[tokio::test]
    async fn test_open_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("deep").join("sessions.db");
        let _store = McpSessionStore::open_at(&path).await.unwrap();
        assert!(path.exists());
    }

    /// Compile-time check that the error mapping composes.
    #[allow(dead_code)]
    fn assert_storage_error_is_boxable(e: StorageError) -> SessionStoreError {
        Box::new(e)
    }
}

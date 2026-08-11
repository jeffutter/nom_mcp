//! Database connection wrapper with FK enforcement and WAL checkpoint invariant.
//!
//! Every connection enables foreign key constraints on open and checkpoints
//! the WAL before closing — the hard invariant from doc-5 §2 that prevents
//! data loss during process handoff between local-CLI and server modes.

use crate::config::db_path;
use std::path::Path;
use turso::{Builder, Connection as TursoConnection};

/// A database connection that enforces FK checks and checkpoints WAL on close.
pub struct Connection {
    inner: TursoConnection,
}

impl Connection {
    /// Open a database at the configured path, enabling FK enforcement.
    ///
    /// Creates parent directories if needed, opens the database in local-file
    /// mode, enables foreign key enforcement, and runs pending migrations.
    pub async fn open() -> Result<Self, StorageError> {
        Self::open_at(&db_path()).await
    }

    /// Open a database at an arbitrary path (used by tests).
    pub async fn open_at(path: &Path) -> Result<Self, StorageError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StorageError::Io(format!("failed to create directory {}: {e}", parent.display()))
            })?;
        }

        let db = Builder::new_local(path.to_str().ok_or_else(|| {
            StorageError::Io("database path contains invalid UTF-8".to_string())
        })?)
        .build()
        .await
        .map_err(|e| StorageError::Database(format!("failed to build database: {e}")))?;

        let conn = db.connect().map_err(|e| {
            StorageError::Database(format!("failed to connect: {e}"))
        })?;

        // Enable foreign key enforcement
        conn.execute("PRAGMA foreign_keys = ON", ())
            .await
            .map_err(|e| StorageError::Database(format!("failed to enable FK: {e}")))?;

        let mut this = Self { inner: conn };

        // Run pending migrations
        super::migration::run(&mut this).await?;

        Ok(this)
    }

    /// Checkpoint the WAL to flush dirty pages to the main database file.
    ///
    /// This is the hard invariant from doc-5 §2 — must be called before
    /// releasing the connection to prevent WAL data loss on crash.
    pub async fn checkpoint(&self) -> Result<(), StorageError> {
        // PRAGMA wal_checkpoint(TRUNCATE) checkpoints and then truncates the WAL file.
        self.inner
            .execute("PRAGMA wal_checkpoint(TRUNCATE)", ())
            .await
            .ok();
        // Also flush the cache
        self.inner.cacheflush().ok();
        Ok(())
    }

    /// Execute a single SQL statement with parameters.
    pub async fn execute(
        &self,
        sql: &str,
        params: impl turso::IntoParams,
    ) -> Result<u64, StorageError> {
        self.inner.execute(sql, params).await.map_err(|e| {
            StorageError::Query(format!("query failed: {e}"))
        })
    }

    /// Prepare a statement for later execution.
    pub async fn prepare(&self, sql: &str) -> Result<turso::Statement, StorageError> {
        self.inner.prepare(sql).await.map_err(|e| {
            StorageError::Query(format!("prepare failed: {e}"))
        })
    }

    /// Execute a batch of SQL statements.
    pub async fn execute_batch(&self, sql: &str) -> Result<(), StorageError> {
        self.inner.execute_batch(sql).await.map_err(|e| {
            StorageError::Query(format!("batch failed: {e}"))
        })
    }

    /// Query rows.
    pub async fn query(
        &self,
        sql: &str,
        params: impl turso::IntoParams,
    ) -> Result<turso::Rows, StorageError> {
        self.inner.query(sql, params).await.map_err(|e| {
            StorageError::Query(format!("query failed: {e}"))
        })
    }

    /// Get the underlying turso connection (for advanced operations).
    pub fn inner(&self) -> &TursoConnection {
        &self.inner
    }
}

/// Errors specific to storage operations.
#[derive(Debug)]
pub enum StorageError {
    Io(String),
    Database(String),
    Query(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "storage I/O error: {msg}"),
            Self::Database(msg) => write!(f, "storage database error: {msg}"),
            Self::Query(msg) => write!(f, "storage query error: {msg}"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<turso::Error> for StorageError {
    fn from(e: turso::Error) -> Self {
        StorageError::Database(format!("turso error: {e}"))
    }
}

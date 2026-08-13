//! Migration runner using the geni pattern — thin, no framework overhead.
//!
//! Embeds migration SQL as a compile-time string, tracks applied versions
//! with SHA-256 hash in `_migrations` table. Runs atomically in a transaction.

use super::connection::{Connection, StorageError};

/// Initial migration SQL (v1).
const MIGRATION_V1: &str = include_str!("schema.sql");

/// SHA-256 hash of v1 migration SQL (computed at build time via sha2 crate,
/// but we compute it at runtime here to avoid another dependency for now).
fn hash_v1() -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let mut hasher = Sha256::new();
    hasher.update(MIGRATION_V1);
    let result = hasher.finalize();
    let mut s = String::with_capacity(result.len() * 2);
    for &byte in &result {
        write!(s, "{byte:02x}").unwrap();
    }
    s
}

/// Run pending migrations against the connection.
///
/// Algorithm:
/// 1. Disable FK checks during DDL
/// 2. BEGIN TRANSACTION
/// 3. Create _migrations table if not exists
/// 4. Check current max version from _migrations
/// 5. For each pending migration: execute SQL, INSERT version + hash into _migrations
/// 6. COMMIT
/// 7. Re-enable FK checks
/// 8. Checkpoint WAL after migration completes
pub async fn run(conn: &mut Connection) -> Result<(), StorageError> {
    // Disable FK checks during DDL (SQLite requires this for schema changes)
    conn.execute("PRAGMA foreign_keys = OFF", ()).await?;

    // Begin transaction
    conn.execute("BEGIN TRANSACTION", ()).await?;

    // Create _migrations table if it doesn't exist
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _migrations ( \
         version INTEGER PRIMARY KEY, \
         hash TEXT NOT NULL, \
         applied_at TEXT NOT NULL DEFAULT (datetime('now')) \
         )",
        (),
    )
    .await?;

    // Check current max version
    let current_version = get_max_version(conn).await?;

    // Apply pending migrations
    apply_migration(conn, current_version).await?;

    // Commit transaction
    conn.execute("COMMIT", ()).await?;

    // Re-enable FK checks
    conn.execute("PRAGMA foreign_keys = ON", ()).await?;

    // Checkpoint WAL after migration completes
    conn.checkpoint().await?;

    Ok(())
}

async fn get_max_version(conn: &Connection) -> Result<i32, StorageError> {
    let mut stmt = conn.prepare("SELECT MAX(version) FROM _migrations").await?;
    let mut rows = stmt
        .query(())
        .await
        .map_err(|e| StorageError::Query(format!("failed to query max version: {e}")))?;

    if let Some(row) = rows
        .next()
        .await
        .map_err(|e| StorageError::Query(format!("failed to read row: {e}")))?
    {
        let value = row
            .get_value(0)
            .map_err(|e| StorageError::Query(format!("failed to get value: {e}")))?;
        match value {
            turso::Value::Integer(v) => Ok(v as i32),
            turso::Value::Null => Ok(0),
            other => Err(StorageError::Query(format!(
                "unexpected value type for max version: {:?}",
                other
            ))),
        }
    } else {
        Ok(0)
    }
}

async fn apply_migration(conn: &mut Connection, current_version: i32) -> Result<(), StorageError> {
    // Currently only v1 exists. Future migrations are added here.
    if current_version >= 1 {
        return Ok(());
    }

    // Execute v1 migration SQL
    conn.execute_batch(MIGRATION_V1).await?;

    // Record migration version and hash
    let hash = hash_v1();
    conn.execute(
        "INSERT OR IGNORE INTO _migrations (version, hash) VALUES (1, ?)",
        (hash.as_str(),),
    )
    .await?;

    Ok(())
}

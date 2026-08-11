//! Integration tests for storage module — run against temp-file databases.

use super::{Connection, StorageError};
use tempfile::TempDir;
use tokio::test;

/// Helper: create a temporary directory and open a connection at a path inside it.
async fn open_temp_db() -> Result<(Connection, TempDir), StorageError> {
    let dir = TempDir::with_prefix("nom_test").unwrap();
    let db_path = dir.path().join("test.db");
    let conn = Connection::open_at(&db_path).await?;
    Ok((conn, dir))
}

#[tokio::test]
async fn test_all_six_tables_created() -> Result<(), StorageError> {
    let (conn, _dir) = open_temp_db().await?;

    // Verify all six domain tables exist
    let tables = [
        "foods",
        "meals",
        "portions",
        "weight_entries",
        "goals",
        "settings",
    ];

    for table in &tables {
        let mut stmt = conn.prepare(&format!(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='{}'",
            table
        ))
        .await?;
        let mut rows = stmt.query(()).await?;
        let row = rows.next().await?.expect("should have a row");
        let count = match row.get_value(0)? {
            turso::Value::Integer(v) => v as i64,
            other => panic!("unexpected value type: {:?}", other),
        };
        assert_eq!(count, 1, "table {} should exist", table);
    }

    Ok(())
}

#[tokio::test]
async fn test_indexes_exist() -> Result<(), StorageError> {
    let (conn, _dir) = open_temp_db().await?;

    let indexes = [
        ("idx_meals_logged_date", "meals"),
        ("idx_portions_meal_id", "portions"),
        ("idx_weight_entries_logged_date", "weight_entries"),
        ("idx_goals_effective_from", "goals"),
    ];

    for (name, tbl) in &indexes {
        let mut stmt = conn.prepare(&format!(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='{}' AND tbl_name='{}'",
            name, tbl
        ))
        .await?;
        let mut rows = stmt.query(()).await?;
        let row = rows.next().await?.expect("should have a row");
        let count = match row.get_value(0)? {
            turso::Value::Integer(v) => v as i64,
            other => panic!("unexpected value type: {:?}", other),
        };
        assert_eq!(count, 1, "index {} on {} should exist", name, tbl);
    }

    Ok(())
}

#[tokio::test]
async fn test_fk_enforcement_active() -> Result<(), StorageError> {
    let (conn, _dir) = open_temp_db().await?;

    // Verify foreign_keys pragma is ON
    let mut stmt = conn.prepare("PRAGMA foreign_keys").await?;
    let mut rows = stmt.query(()).await?;
    let row = rows.next().await?.expect("should have a row");
    let enabled = match row.get_value(0)? {
        turso::Value::Integer(v) => v == 1,
        other => panic!("unexpected value type: {:?}", other),
    };
    assert!(enabled, "foreign keys should be enabled");

    Ok(())
}

#[tokio::test]
async fn test_migrations_table_has_version_entry() -> Result<(), StorageError> {
    let (conn, _dir) = open_temp_db().await?;

    // Check _migrations table has exactly one entry for version 1
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM _migrations WHERE version = 1").await?;
    let mut rows = stmt.query(()).await?;
    let row = rows.next().await?.expect("should have a row");
    let count = match row.get_value(0)? {
        turso::Value::Integer(v) => v as i64,
        other => panic!("unexpected value type: {:?}", other),
    };
    assert_eq!(count, 1, "_migrations should have exactly one entry for version 1");

    // Verify hash is non-empty
    let mut stmt = conn.prepare("SELECT hash FROM _migrations WHERE version = 1").await?;
    let mut rows = stmt.query(()).await?;
    let row = rows.next().await?.expect("should have a row");
    let hash = match row.get_value(0)? {
        turso::Value::Text(h) => h,
        other => panic!("unexpected value type: {:?}", other),
    };
    assert!(!hash.is_empty(), "migration hash should not be empty");

    Ok(())
}

#[tokio::test]
async fn test_migration_idempotency() -> Result<(), StorageError> {
    let dir = TempDir::with_prefix("nom_test").unwrap();
    let db_path = dir.path().join("test.db");

    // Open first time — runs migrations
    {
        let _conn = Connection::open_at(&db_path).await?;
    }

    // Open second time — should NOT error (idempotent)
    {
        let _conn = Connection::open_at(&db_path).await?;
    }

    // Verify _migrations still has exactly one entry
    let conn = Connection::open_at(&db_path).await?;
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM _migrations").await?;
    let mut rows = stmt.query(()).await?;
    let row = rows.next().await?.expect("should have a row");
    let count = match row.get_value(0)? {
        turso::Value::Integer(v) => v as i64,
        other => panic!("unexpected value type: {:?}", other),
    };
    assert_eq!(count, 1, "should still have exactly one migration entry after re-open");

    Ok(())
}

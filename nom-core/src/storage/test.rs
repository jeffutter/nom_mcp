//! Integration tests for storage module — run against temp-file databases.

use super::{Connection, StorageError};
use std::path::PathBuf;
use tempfile::TempDir;

/// Reusable temp database fixture. Keeps the TempDir alive until dropped.
pub struct TempDb {
    pub path: PathBuf,
    _dir: TempDir,
}

impl TempDb {
    pub async fn new() -> Self {
        let dir = TempDir::with_prefix("nom_test").unwrap();
        let path = dir.path().join("test.db");
        // Open to trigger migrations, then drop the connection
        let _conn = Connection::open_at(&path)
            .await
            .expect("failed to open temp db");
        Self { path, _dir: dir }
    }
}

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
        let mut stmt = conn
            .prepare(&format!(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='{}'",
                table
            ))
            .await?;
        let mut rows = stmt.query(()).await?;
        let row = rows.next().await?.expect("should have a row");
        let count = match row.get_value(0)? {
            turso::Value::Integer(v) => v,
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
            turso::Value::Integer(v) => v,
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

    // Both registered migrations must be recorded exactly once each.
    for version in [1_i64, 2] {
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM _migrations WHERE version = ?")
            .await?;
        let mut rows = stmt.query((version,)).await?;
        let row = rows.next().await?.expect("should have a row");
        let count = match row.get_value(0)? {
            turso::Value::Integer(v) => v,
            other => panic!("unexpected value type: {:?}", other),
        };
        assert_eq!(
            count, 1,
            "_migrations should have exactly one entry for version {version}"
        );

        // Verify hash is non-empty
        let mut stmt = conn
            .prepare("SELECT hash FROM _migrations WHERE version = ?")
            .await?;
        let mut rows = stmt.query((version,)).await?;
        let row = rows.next().await?.expect("should have a row");
        let hash = match row.get_value(0)? {
            turso::Value::Text(h) => h,
            other => panic!("unexpected value type: {:?}", other),
        };
        assert!(!hash.is_empty(), "hash for v{version} should not be empty");
    }

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

    // Verify _migrations still has exactly two entries (one per migration)
    let conn = Connection::open_at(&db_path).await?;
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM _migrations").await?;
    let mut rows = stmt.query(()).await?;
    let row = rows.next().await?.expect("should have a row");
    let count = match row.get_value(0)? {
        turso::Value::Integer(v) => v,
        other => panic!("unexpected value type: {:?}", other),
    };
    assert_eq!(
        count, 2,
        "should still have exactly two migration entries after re-open"
    );

    Ok(())
}

/// Reads `PRAGMA table_info(meals)` and returns the column names in order.
async fn meal_column_names(conn: &Connection) -> Result<Vec<String>, StorageError> {
    let mut stmt = conn.prepare("PRAGMA table_info(meals)").await?;
    let mut rows = stmt.query(()).await?;
    let mut names = Vec::new();
    while let Some(row) = rows.next().await? {
        match row.get_value(1)? {
            turso::Value::Text(name) => names.push(name),
            other => panic!("unexpected value type for column name: {:?}", other),
        }
    }
    Ok(names)
}

#[tokio::test]
async fn test_meal_type_is_last_column_on_fresh_db() -> Result<(), StorageError> {
    let (conn, _dir) = open_temp_db().await?;

    let columns = meal_column_names(&conn).await?;

    // v2 appends meal_type as the LAST column; the v1 columns keep their
    // original order untouched ahead of it.
    let expected_v1 = [
        "id",
        "logged_at",
        "logged_date",
        "total_calories",
        "total_protein_g",
        "total_carbs_g",
        "total_fat_g",
        "total_fiber_g",
        "adjustment_calories",
        "adjustment_protein_g",
        "adjustment_carbs_g",
        "adjustment_fat_g",
        "adjustment_fiber_g",
    ];
    assert_eq!(
        columns,
        expected_v1
            .iter()
            .map(|s| s.to_string())
            .chain(["meal_type".to_string()])
            .collect::<Vec<_>>(),
        "meal_type must be appended last without shifting any v1 column"
    );

    Ok(())
}

#[tokio::test]
async fn test_v2_not_reapplied_on_reopen() -> Result<(), StorageError> {
    let dir = TempDir::with_prefix("nom_test").unwrap();
    let db_path = dir.path().join("test.db");

    // First open applies v1 + v2; second open must apply nothing (a reapplied
    // ALTER TABLE ... ADD COLUMN would fail outright).
    drop(Connection::open_at(&db_path).await?);
    drop(Connection::open_at(&db_path).await?);

    let conn = Connection::open_at(&db_path).await?;
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM _migrations WHERE version = 2")
        .await?;
    let mut rows = stmt.query(()).await?;
    let row = rows.next().await?.expect("should have a row");
    let count = match row.get_value(0)? {
        turso::Value::Integer(v) => v,
        other => panic!("unexpected value type: {:?}", other),
    };
    assert_eq!(count, 1, "v2 must be recorded exactly once across reopens");

    let columns = meal_column_names(&conn).await?;
    assert_eq!(
        columns.last().map(String::as_str),
        Some("meal_type"),
        "meal_type must remain the last column after reopen"
    );

    Ok(())
}

#[tokio::test]
async fn test_legacy_v1_db_upgrades_with_rows_intact() -> Result<(), StorageError> {
    let (conn, dir) = open_temp_db().await?;

    // Seed one row using only v1 columns.
    conn.execute(
        "INSERT INTO meals (logged_at, logged_date, total_calories) VALUES (?, ?, ?)",
        ("2026-09-07T08:00:00Z", "2026-09-07", 250.0_f64),
    )
    .await?;

    // Simulate a v1-only database: remove the v2 column and its migration row.
    conn.execute("ALTER TABLE meals DROP COLUMN meal_type", ())
        .await?;
    conn.execute("DELETE FROM _migrations WHERE version = 2", ())
        .await?;
    conn.checkpoint().await?;
    drop(conn);

    // Reopen through the normal path — the runner must apply only v2.
    let conn = Connection::open_at(&dir.path().join("test.db")).await?;

    let columns = meal_column_names(&conn).await?;
    assert_eq!(
        columns.last().map(String::as_str),
        Some("meal_type"),
        "meal_type must be appended on upgrade from a v1-only database"
    );

    // The pre-existing row survives with meal_type NULL (CHECK is tri-valued).
    let mut stmt = conn
        .prepare("SELECT total_calories, meal_type FROM meals")
        .await?;
    let mut rows = stmt.query(()).await?;
    let row = rows.next().await?.expect("seeded row should survive");
    let calories = match row.get_value(0)? {
        turso::Value::Real(v) => v,
        other => panic!("unexpected value type: {:?}", other),
    };
    assert_eq!(calories, 250.0);
    assert!(
        matches!(row.get_value(1)?, turso::Value::Null),
        "pre-existing rows must keep meal_type NULL (no backfill)"
    );
    assert!(rows.next().await?.is_none(), "exactly one seeded row");

    // _migrations ends with exactly the two rows, both hashed.
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM _migrations").await?;
    let mut rows = stmt.query(()).await?;
    let row = rows.next().await?.expect("should have a row");
    let count = match row.get_value(0)? {
        turso::Value::Integer(v) => v,
        other => panic!("unexpected value type: {:?}", other),
    };
    assert_eq!(
        count, 2,
        "upgraded DB should hold exactly two migration rows"
    );

    Ok(())
}

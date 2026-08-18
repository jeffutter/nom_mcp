//! E2E for `seed_data` (TASK-54): seed a throwaway DB through the real CLI
//! binary, then query it via the `NOM_MCP_DB_PATH` env override — encoding
//! AC#1/#2/#3 plus the goal-progress half of AC#4 as a test and exercising
//! the real CLI surface plus the env override together.
//!
//! `get_weekly_progress` is intentionally NOT exercised here: it is
//! `Surfaces::MCP`-only (no CLI/HTTP route by design), so its seeded-data
//! behavior is covered by the direct-operation test in
//! `nom-core/src/seed/mod.rs` (`test_weekly_progress_on_seeded_db`).
//!
//! The env var is passed to the spawned child only; this process's
//! environment is never mutated (`std::env::set_var` is unsafe in edition
//! 2024).

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn run_cli(args: &[&str], db_path: Option<&Path>) -> (bool, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nom-mcp"));
    cmd.args(args);
    if let Some(p) = db_path {
        cmd.env("NOM_MCP_DB_PATH", p);
    }
    let out = cmd.output().expect("failed to spawn nom-mcp");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn test_seed_then_query_goal_progress() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("nom.db");
    let db_str = db.to_string_lossy().to_string();

    // 1. Seed a fresh throwaway DB (explicit path arg, no env override)
    let (ok, stdout, stderr) = run_cli(&["seed_data", "--path", &db_str], None);
    assert!(ok, "seed_data failed: {stderr}");
    let seed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(seed["db_path"], db_str);
    assert_eq!(seed["days"], 7);
    assert_eq!(seed["foods"], 7);
    assert_eq!(seed["meals"], 18);
    assert_eq!(seed["portions"], 37);
    assert_eq!(seed["weight_entries"], 7);

    // 2. get_goal_progress against the seeded DB via NOM_MCP_DB_PATH
    let (ok, stdout, stderr) = run_cli(&["get_goal_progress"], Some(&db));
    assert!(ok, "get_goal_progress failed: {stderr}");
    let progress: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let nutrients = ["calories", "protein_g", "carbs_g", "fat_g", "fiber_g"];
    let statuses: std::collections::HashSet<&str> = nutrients
        .iter()
        .map(|n| {
            // All five nutrients must have non-null consumed AND target
            assert!(
                progress[n]["consumed"].as_f64().is_some(),
                "{n}: consumed should be non-null"
            );
            assert!(
                progress[n]["target"].as_f64().is_some(),
                "{n}: target should be non-null"
            );
            progress[n]["status"]
                .as_str()
                .unwrap_or_else(|| panic!("{n}: status should be present"))
        })
        .collect();
    assert!(
        statuses.contains("under") && statuses.contains("met") && statuses.contains("over"),
        "statuses must span under/met/over, got {statuses:?}"
    );

    // 3. Re-seeding resets to the same known state (repeatable)
    let (ok, _, stderr) = run_cli(&["seed_data", "--path", &db_str], None);
    assert!(ok, "re-seed failed: {stderr}");
    let (ok, stdout, stderr) = run_cli(&["get_goal_progress"], Some(&db));
    assert!(ok, "post-reseed get_goal_progress failed: {stderr}");
    let again: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        progress, again,
        "re-seeded DB must yield identical progress"
    );
}

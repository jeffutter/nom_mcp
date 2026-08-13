//! Spawns the `lock_holder` helper binary to verify `probe_db_lock` detects
//! a write lock held by another process.
//!
//! This lives as an integration test (rather than a unit test inside
//! `storage::lock_probe`) because only Cargo's integration-test targets get
//! `CARGO_BIN_EXE_lock_holder` set at compile time — that env var is both
//! the guaranteed path to the binary and what makes Cargo build it before
//! this test runs.

use nom_core::storage::lock_probe::probe_db_lock;
use tempfile::TempDir;

#[test]
fn test_probe_locked_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("locked.db");
    // Create the file first so the child can open it
    std::fs::File::create(&path).unwrap();

    let path_str = path.to_string_lossy().to_string();
    let helper_path = env!("CARGO_BIN_EXE_lock_holder");

    // Spawn the lock_holder binary
    let mut child = std::process::Command::new(helper_path)
        .env("NOM_HOLD_LOCK_PATH", &path_str)
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn lock_holder: {}", e));

    // Give the child time to start up and acquire the lock
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Verify child is still running (it should be sleeping)
    match child.try_wait() {
        Ok(Some(status)) => panic!(
            "child exited prematurely with status {:?} — lock setup failed",
            status
        ),
        Err(e) => panic!("try_wait failed: {}", e),
        Ok(None) => {} // child still running, good
    }

    // Probe should detect the lock
    let is_locked = probe_db_lock(&path).expect("probe should succeed");
    assert!(is_locked, "probe should detect child's write lock");

    // Kill the child and verify lock is released
    child.kill().expect("kill child");
    child.wait().expect("wait for child");
    std::thread::sleep(std::time::Duration::from_millis(100));

    let is_locked = probe_db_lock(&path).expect("probe after child exit");
    assert!(!is_locked, "lock should be released after child exits");
}

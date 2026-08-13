//! Spawns the `lock_holder` helper binary to verify `probe_db_lock` detects
//! a write lock held by another process.
//!
//! This lives as an integration test (rather than a unit test inside
//! `storage::lock_probe`) because only Cargo's integration-test targets get
//! `CARGO_BIN_EXE_lock_holder` set at compile time — that env var is both
//! the guaranteed path to the binary and what makes Cargo build it before
//! this test runs.

use nom_core::storage::lock_probe::probe_db_lock;
use std::io::Read;
use std::sync::mpsc;
use tempfile::TempDir;

/// RAII guard that kills the child process on drop.
/// Ensures no orphaned processes leak if the test panics between spawn and explicit kill.
struct ChildGuard(Option<std::process::Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn test_probe_locked_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("locked.db");
    // Create the file first so the child can open it
    std::fs::File::create(&path).unwrap();

    let path_str = path.to_string_lossy().to_string();
    let helper_path = env!("CARGO_BIN_EXE_lock_holder");

    // Channel for child-to-parent ack signal
    let (tx, rx) = mpsc::channel();

    // Spawn the lock_holder binary with piped stdout
    let mut child = std::process::Command::new(helper_path)
        .env("NOM_HOLD_LOCK_PATH", &path_str)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn lock_holder: {}", e));

    // Take stdout pipe BEFORE wrapping in guard (Child doesn't implement Copy)
    let mut stdout = child.stdout.take().expect("child has piped stdout");

    // RAII safety net: kills child if we panic before explicit cleanup
    let mut guard = ChildGuard(Some(child));

    // Reader thread: blocks on exactly one byte from child stdout
    let reader_thread = std::thread::spawn(move || {
        let mut buf = [0u8];
        let result = stdout.read_exact(&mut buf);
        let _ = tx.send(result.map(|_| ()).map_err(|e| e.to_string()));
    });

    // Wait for ack with bounded timeout — replaces fixed 200ms sleep
    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(())) => {} // got the ack byte — child holds the lock
        Ok(Err(e)) => panic!("child stdout read error: {}", e),
        Err(_) => panic!("child did not ack lock acquisition within 5s"),
    }

    // Guard stays armed through these assertions — any panic here still needs
    // the child killed, so defuse only right before the explicit kill below.
    let child_ref = guard.0.as_mut().expect("guard holds child");

    // Verify child is still running (it should be sleeping)
    match child_ref.try_wait() {
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

    // Defuse RAII guard — we manage the child explicitly from here on
    let mut child = guard.0.take().expect("guard was defused");

    // Kill the child and verify lock is released
    child.kill().expect("kill child");
    child.wait().expect("wait for child");
    std::thread::sleep(std::time::Duration::from_millis(100));

    let is_locked = probe_db_lock(&path).expect("probe after child exit");
    assert!(!is_locked, "lock should be released after child exits");

    // Join reader thread (no-op since channel is already consumed)
    let _ = reader_thread.join();
}

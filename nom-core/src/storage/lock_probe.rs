//! POSIX advisory lock probe for the local database file.
//!
//! Uses the same `fcntl` mechanism that Turso uses internally. Opens the
//! database file, calls `F_GETLK` with `F_WRLCK`, and checks if another
//! process holds a write lock. Returns without acquiring any lock (RAII —
//! the file handle drops immediately).

use std::path::Path;

/// Probe whether another process holds a write lock on the given file.
///
/// Returns `Ok(true)` if the lock is held by another process, `Ok(false)`
/// if the file is free to open. Errors propagate from the underlying I/O
/// (e.g., file not found).
pub fn probe_db_lock(path: &Path) -> Result<bool, std::io::Error> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;

    let fd = std::os::fd::AsRawFd::as_raw_fd(&file);
    let mut flock = libc::flock {
        l_type: libc::F_WRLCK as i16,
        l_whence: libc::SEEK_SET as i16,
        l_start: 0,
        l_len: 0,
        l_pid: 0,
    };

    // F_GETLK does not block — it returns information about an existing lock
    // or fills in a zeroed struct if no lock exists.
    unsafe {
        libc::fcntl(fd, libc::F_GETLK, &mut flock);
    }

    // If l_pid != 0, another process holds the lock
    Ok(flock.l_pid != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Resolve the path to the `lock_holder` binary built by cargo.
    ///
    /// `cargo test` builds every `[[bin]]` target before running tests, so the
    /// binary already exists by the time this runs — no need to shell out to
    /// `cargo build` ourselves. The test binary lives at
    /// `target/<profile>/deps/<name>-<hash>`; `lock_holder` is a sibling one
    /// level up, at `target/<profile>/lock_holder`.
    fn find_lock_holder() -> std::path::PathBuf {
        let test_exe = std::env::current_exe().expect("current_exe should resolve");
        let profile_dir = test_exe
            .parent() // .../target/<profile>/deps
            .and_then(|p| p.parent()) // .../target/<profile>
            .unwrap_or_else(|| panic!("could not determine profile dir from {:?}", test_exe));
        let bin_path = profile_dir.join("lock_holder");
        if !bin_path.exists() {
            panic!("could not find lock_holder binary at {:?}", bin_path);
        }
        bin_path
    }

    #[test]
    fn test_probe_unlocked_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        std::fs::File::create(&path).unwrap();

        let is_locked = probe_db_lock(&path).unwrap();
        assert!(!is_locked, "fresh file should not be locked");
    }

    #[test]
    fn test_probe_missing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.db");

        let result = probe_db_lock(&path);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
    }

    /// Spawn a child process that holds a write lock on a temp file, then
    /// verify the probe detects it. After the child exits, verify the lock
    /// is released.
    ///
    /// Uses the dedicated `lock_holder` binary — no external `rustc` needed,
    /// no ad-hoc compilation. Fails loudly if the helper cannot be spawned.
    #[test]
    fn test_probe_locked_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("locked.db");
        // Create the file first so the child can open it
        std::fs::File::create(&path).unwrap();

        let path_str = path.to_string_lossy().to_string();
        let helper_path = find_lock_holder();

        // Spawn the lock_holder binary
        let mut child = std::process::Command::new(&helper_path)
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
}

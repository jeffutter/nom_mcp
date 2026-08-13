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

// The test that spawns the `lock_holder` helper binary lives in
// `nom-core/tests/lock_probe_integration.rs` instead of here: only Cargo's
// integration-test targets get `CARGO_BIN_EXE_lock_holder` set (and thus
// get the binary built automatically before the test runs). Unit tests
// compiled into the lib itself don't.
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
}

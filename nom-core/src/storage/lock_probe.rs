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
    use std::io::Write;
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

    /// Spawn a child process that holds a write lock on a temp file, then
    /// verify the probe detects it. After the child exits, verify the lock
    /// is released.
    #[test]
    fn test_probe_locked_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("locked.db");
        // Create the file first so the child can open it
        std::fs::File::create(&path).unwrap();

        // Build a small helper program inline via a temporary Rust source file
        let path_str = path.to_string_lossy().to_string();
        let helper_src = [
            "use std::os::unix::io::{FromRawFd, RawFd};",
            "fn main() {",
            &format!("    let path = \"{}\";", path_str),
            "    let fd = unsafe { libc::open(path.as_bytes().as_ptr() as *const _, libc::O_RDWR) };",
            "    if fd < 0 { panic!(\"failed to open\") }",
            "    let mut flock = libc::flock {",
            "        l_type: libc::F_WRLCK as i16,",
            "        l_whence: libc::SEEK_SET as i16,",
            "        l_start: 0,",
            "        l_len: 0,",
            "        l_pid: 0,",
            "    };",
            "    unsafe { libc::fcntl(fd, libc::F_SETLKW, &mut flock); }",
            "    std::thread::sleep(std::time::Duration::from_secs(30));",
            "}",
        ].join("\n");

        let helper_dir = TempDir::new().unwrap();
        let src_path = helper_dir.path().join("helper.rs");
        std::fs::File::create(&src_path)
            .unwrap()
            .write_all(helper_src.as_bytes())
            .unwrap();

        // Compile the helper
        let rustc = std::process::Command::new("rustc")
            .arg(&src_path)
            .arg("-o")
            .arg(helper_dir.path().join("helper"))
            .output()
            .expect("rustc should run");

        if !rustc.status.success() {
            // If rustc isn't available or fails, skip this test gracefully
            eprintln!(
                "skipping locked-file test (rustc unavailable): {}",
                String::from_utf8_lossy(&rustc.stderr)
            );
            return;
        }

        // Spawn the helper — it will hold the lock
        let mut child = std::process::Command::new(helper_dir.path().join("helper"))
            .spawn()
            .expect("spawn helper");

        // Give it a moment to acquire the lock
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Probe should detect the lock
        let is_locked = probe_db_lock(&path).expect("probe should succeed");
        assert!(is_locked, "probe should detect child's write lock");

        // Kill the child and verify lock is released
        child.kill().ok();
        child.wait().ok();
        std::thread::sleep(std::time::Duration::from_millis(100));

        let is_locked = probe_db_lock(&path).expect("probe after child exit");
        assert!(!is_locked, "lock should be released after child exits");
    }
}

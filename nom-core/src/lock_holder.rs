//! Helper binary for lock_probe tests.
//! Acquires an exclusive write lock on the file given via NOM_HOLD_LOCK_PATH
//! and sleeps until killed.

use std::io::Write;
use std::path::Path;

fn hold_lock(path: &Path) {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap_or_else(|e| panic!("failed to open {}: {}", path.display(), e));

    let fd = std::os::fd::AsRawFd::as_raw_fd(&file);
    let mut flock = libc::flock {
        l_type: libc::F_WRLCK as i16,
        l_whence: libc::SEEK_SET as i16,
        l_start: 0,
        l_len: 0,
        l_pid: 0,
    };
    let ret = unsafe { libc::fcntl(fd, libc::F_SETLKW, &mut flock) };
    if ret != 0 {
        let err = std::io::Error::last_os_error();
        panic!("F_SETLKW failed on {}: {}", path.display(), err);
    }
    // Ack parent: we hold the lock now
    std::io::stdout().write_all(b"1").unwrap();
    std::io::stdout().flush().unwrap();
    // Keep sleeping; the file handle stays alive keeping the lock held.
    std::thread::sleep(std::time::Duration::from_secs(300));
}

fn main() {
    let path_str = std::env::var("NOM_HOLD_LOCK_PATH").expect("NOM_HOLD_LOCK_PATH must be set");
    hold_lock(Path::new(&path_str));
}

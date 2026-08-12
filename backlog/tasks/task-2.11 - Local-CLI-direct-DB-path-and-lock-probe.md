---
id: TASK-2.11
title: Local-CLI direct-DB path and lock probe
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:24'
updated_date: '2026-08-12 04:16'
labels:
  - planned
dependencies:
  - TASK-2.5
  - TASK-2.7
type: feature
ordinal: 30000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
The main binary's local CLI always executes Operations in-process against the local Turso file, first-class and top-level alongside 'serve' — not a runtime decision, no remote fallback (that's nom-mcp-remote's job). Given the clean-close/checkpoint invariant from the storage schema design, local-CLI adds a runtime lock probe (same POSIX advisory lock turso already takes) before opening the DB directly, failing fast rather than risking silent WAL corruption if the server appears to hold it. The lock-probe rejection is an ordinary Conflict error with reason 'local_db_locked', rendered with a CLI-specific message ('server is running — stop it or use the remote-CLI instead').

See doc-5 §2 and §3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 local CLI subcommands execute in-process against the local DB file with no network path
- [x] #2 opening the DB directly first probes the advisory lock and fails fast with a Conflict/local_db_locked error if held
- [x] #3 the CLI-specific message directs the user to stop the server or use nom-mcp-remote
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan: Local-CLI Direct-DB Path & Lock Probe

### Overview

Implement the lock-probe mechanism that prevents local-CLI from opening the database when the server already holds an advisory lock on it. Uses POSIX `fcntl` to probe the .db file BEFORE constructing a Turso database, failing fast with a Conflict/local_db_locked error if another process holds a write lock.

### Dependencies

- TASK-2.5 (Storage schema) — Done: provides Connection, StorageError, db_path()
- TASK-2.7 (Operation trait/registry) — Done: provides execute_from_args wiring point
- Error infrastructure — Already in place: `ErrorData::conflict("local_db_locked")` renders correctly per existing tests

### Phase 1: Add libc dependency (if not already present)

**File: nom-core/Cargo.toml**

Check if `libc` is already a direct dependency (turso pulls it transitively). If not, add it:
```toml
libc = "0.2"
```

Turso already uses `libc` internally for its own fcntl-based locking, so it is almost certainly available transitively. Verify with `cargo tree | grep libc`.

### Phase 2: Lock probe function

**New file: nom-core/src/storage/lock_probe.rs**

```rust
/// Probe whether another process holds a write lock on the database file.
///
/// Uses POSIX advisory locks (same mechanism Turso uses). Opens the file,
/// calls F_GETLK with l_type=F_WRLCK, and checks if l_pid != 0 (locked).
///
/// Returns Ok(true) if the lock is held by another process, Ok(false) if free.
pub fn probe_db_lock(path: &std::path::Path) -> Result<bool, std::io::Error> { ... }
```

Implementation details:
- Open file with `std::fs::OpenOptions { read: true, write: true }.open(path)`
- Use `unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETLK, &mut flock) }` where `flock.l_type = libc::F_WRLCK`
- After call: if `flock.l_pid != 0`, another process holds the lock
- File handle auto-drops (RAII), no lock acquired by this probe
- Works on Linux (OFD locks) and macOS (process-scoped fcntl)

### Phase 3: Integrate into Connection open path

**Modified file: nom-core/src/storage/connection.rs**

Add `probe_lock` parameter or create a separate `Connection::open_with_probe()` method. Best approach: modify `Connection::open_at()` to accept a `probe_lock: bool` flag, defaulting to `true` for safety. When `true`, call `probe_db_lock()` before `Builder::new_local()`.

Alternatively, keep `open_at()` unchanged and add a new `Connection::open_safe()` that does the probe + open sequence. This avoids changing the existing API used by tests (which use temp files with no concurrent access).

Recommended: add `Connection::open_safe()` that calls `probe_db_lock()` then delegates to `open_at()`. Tests continue using `open_at()` directly.

### Phase 4: Wire into CLI execution

**Modified file: nom-mcp/src/main.rs**

In `execute_from_args()`, before building the registry or opening any connection:
```rust
// Probe lock before opening DB
let db_path = config.db_path();
if storage::probe_db_lock(&db_path).map_err(|e| ErrorData::storage_failure(e.to_string()))? {
    return Err(ErrorData::conflict("local_db_locked"));
}
```

The existing `render_error` already handles `Conflict` + `reason: "local_db_locked"` with the user-friendly message ("server is running — stop it or use the remote-CLI instead"). Exit code 5 per doc-5 §10.

### Phase 5: Unit tests

**Tests in lock_probe.rs:**
1. `test_probe_unlocked_file` — probe a fresh temp file, expect `Ok(false)`
2. `test_probe_locked_file` — spawn child process holding fcntl write lock on temp file, verify probe returns `Ok(true)`; child exits, verify probe returns `Ok(false)` again
3. `test_probe_missing_file` — probe a non-existent path, expect `Err(io::ErrorKind::NotFound)`

Child process test pattern: use `std::process::Command` to spawn a helper binary or inline script that opens the file and holds a lock via `libc::fcntl(F_SETLK, F_WRLCK)` for a duration long enough for the parent to probe.

### File Change Summary

| File | Action | Description |
|------|--------|-------------|
| `nom-core/Cargo.toml` | Edit | Add `libc` dependency (if needed) |
| `nom-core/src/storage/lock_probe.rs` | New | Lock probe implementation + unit tests |
| `nom-core/src/storage/mod.rs` | Edit | Export lock_probe module |
| `nom-core/src/storage/connection.rs` | Edit | Add `open_safe()` method |
| `nom-mcp/src/main.rs` | Edit | Wire lock probe into `execute_from_args()` |

### Acceptance Criteria Mapping

- AC #1 (in-process execution): Existing `execute_from_args` already runs locally; no network path. Confirmed by current stub returning clock data without HTTP calls.
- AC #2 (lock probe before open): `probe_db_lock()` called before `Builder::new_local()`, returns `Conflict/local_db_locked` if held.
- AC #3 (user-facing message): `render_error` already produces "server is running — stop it or use the remote-CLI instead" for this reason.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation:
- Added libc dependency to nom-core/Cargo.toml for POSIX fcntl access
- Created nom-core/src/storage/lock_probe.rs with probe_db_lock() function using F_GETLK
- Added Connection::open_safe() method that probes lock before opening DB
- Wired lock probe into execute_from_args() in nom-mcp/src/main.rs
- Added 3 unit tests: unlocked file, locked file (via child process), missing file
- All 112 tests pass including 3 new lock_probe tests
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented POSIX advisory lock probe (fcntl F_GETLK) in nom-core/src/storage/lock_probe.rs. Added Connection::open_safe() method that probes before opening DB. Wired lock_probe::probe_db_lock() into execute_from_args() in nom-mcp/src/main.rs — fails fast with Conflict/local_db_locked error if server holds the write lock. All 3 acceptance criteria verified: (1) in-process execution confirmed, (2) lock probe integrated before DB open, (3) user-friendly CLI message via existing ErrorData infrastructure. 112 tests pass including 3 new lock_probe unit tests.
<!-- SECTION:FINAL_SUMMARY:END -->

//! E2E for the remote-CLI nested-JSON fix (TASK-60): seed a throwaway DB
//! through the real CLI binary, start a real `serve http` server against it,
//! then invoke `nom-mcp-remote log_meal` with a raw bracketed-JSON `portions`
//! argument — the exact invocation that returned 400 ("invalid type: string
//! \"[...]\", expected a sequence") before `cli::parse_value` started
//! JSON-decoding values first.
//!
//! No shell is involved (`std::process::Command`), so the brackets and quotes
//! reach `parse_params` verbatim. Env vars are passed to spawned children
//! only; this process's environment is never mutated (`std::env::set_var` is
//! unsafe in edition 2024).

use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command};
use std::time::Duration;
use tempfile::TempDir;

/// Spawn a child with an isolated config home (empty dir) plus extra env,
/// capturing stdout/stderr. The empty `XDG_CONFIG_HOME` keeps every child
/// hermetic: no real user config file can leak into the run.
fn run_with_env(
    bin: &str,
    args: &[&str],
    xdg_config_home: &Path,
    extra_env: &[(&str, &str)],
) -> (bool, String, String) {
    let mut cmd = Command::new(bin);
    cmd.args(args).env("XDG_CONFIG_HOME", xdg_config_home);
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {bin}: {e}"));
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Bind a port, capture the OS-assigned number, release it.
fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind a free port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

#[test]
fn test_remote_cli_logs_meal_with_nested_json_portions() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("nom.db");
    let db_str = db.to_string_lossy().to_string();
    let xdg = TempDir::new().unwrap(); // empty config home for all children

    // 1. Seed a fresh throwaway DB (explicit path arg, no env override).
    //    Seed foods use deterministic ids 1..N, so food_id 1 always exists.
    let (ok, stdout, stderr) = run_with_env(
        env!("CARGO_BIN_EXE_nom-mcp"),
        &["seed_data", "--path", &db_str],
        xdg.path(),
        &[],
    );
    assert!(ok, "seed_data failed: {stderr}\nstdout: {stdout}");

    // 2. Free port + spawn the real HTTP server against the seeded DB.
    let port = free_port();
    let url = format!("http://127.0.0.1:{port}");
    let server = Command::new(env!("CARGO_BIN_EXE_nom-mcp"))
        .args(["serve", "http", "--port", &port.to_string()])
        .env("NOM_MCP_DB_PATH", &db_str)
        .env("XDG_CONFIG_HOME", xdg.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn nom-mcp serve http");

    // Ensure the server is cleaned up even if an assertion below panics.
    struct ServerGuard(Option<Child>);
    impl Drop for ServerGuard {
        fn drop(&mut self) {
            if let Some(mut c) = self.0.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
    let guard = ServerGuard(Some(server));

    // 3. Readiness probe (~10s budget): cheap local read op over HTTP until exit 0.
    let mut last_stderr = String::new();
    let ready = (0..100).any(|_| {
        let (ok, _, stderr) = run_with_env(
            env!("CARGO_BIN_EXE_nom-mcp-remote"),
            &["get_goal_progress"],
            xdg.path(),
            &[("NOM_MCP_remote__server_url", url.as_str())],
        );
        last_stderr = stderr;
        if !ok {
            std::thread::sleep(Duration::from_millis(100));
        }
        ok
    });
    assert!(
        ready,
        "server did not become ready within ~10s; last probe stderr: {last_stderr}"
    );

    // 4. Act: the exact pre-fix failure mode — raw bracketed JSON as one arg.
    let portions_arg = "portions=[{\"food_id\":1,\"quantity\":250,\"quantity_mode\":\"grams\"}]";
    let (ok, stdout, stderr) = run_with_env(
        env!("CARGO_BIN_EXE_nom-mcp-remote"),
        &["log_meal", portions_arg],
        xdg.path(),
        &[("NOM_MCP_remote__server_url", url.as_str())],
    );
    assert!(
        ok,
        "log_meal via remote CLI failed: {stderr}\nstdout: {stdout}"
    );

    // 5. Assert the response shape: {meal_id, logged_at, logged_date, totals}.
    let resp: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON: {e}\n{stdout}"));
    let meal_id = resp["meal_id"]
        .as_i64()
        .expect("meal_id must be an integer");
    assert!(meal_id >= 1, "meal_id must be >= 1, got {meal_id}");
    let logged_date = resp["logged_date"]
        .as_str()
        .expect("logged_date must be a non-empty string");
    assert!(!logged_date.is_empty());
    let calories = resp["totals"]["total_calories"]
        .as_f64()
        .expect("totals.total_calories must be a number");
    assert!(
        calories > 0.0,
        "total_calories must be positive, got {calories}"
    );

    // 6. Structurally-valid but invalid portion (unknown food_id) must fail at
    //    server-side validation, not at request deserialization — proving the
    //    nested JSON now reaches the server intact.
    let bad_portions =
        "portions=[{\"food_id\":999999,\"quantity\":100,\"quantity_mode\":\"grams\"}]";
    let (ok, stdout, stderr) = run_with_env(
        env!("CARGO_BIN_EXE_nom-mcp-remote"),
        &["log_meal", bad_portions],
        xdg.path(),
        &[("NOM_MCP_remote__server_url", url.as_str())],
    );
    assert!(!ok, "unknown food_id must fail; stdout: {stdout}");
    assert!(
        !stderr.trim().is_empty(),
        "failure must render an error on stderr"
    );

    // 7. ServerGuard kills + reaps the server on drop (including panic paths).
    drop(guard);
}

# nom_mcp

A single-user Rust nutrition tracker for logging meals, body weight, and nutrition goals — backed by OpenFoodFacts and USDA FoodData Central for food data, with local-file SQLite (via [turso](https://github.com/tursodatabase/turso)) storage. Exposed identically over MCP (stdio or streamable-HTTP), a local CLI, a REST HTTP API, and a remote-CLI thin client, all driven by one shared `Operation` abstraction (see [AGENTS.md](AGENTS.md) for the architecture).

**Status: v1 complete.** Food/meal/weight/goal/widget tracking, both `serve` transports, the `nom://weekly-summary` MCP resource, and `nom-mcp-remote` are all implemented.

## Domain model

The full glossary lives in [CONTEXT.md](CONTEXT.md). In short: a **Meal** is composed of **Portions**, each a quantity of a catalog **Food** (OpenFoodFacts barcode lookup, USDA FDC whole foods, or a user-defined Custom Food) — nutrients are snapshotted onto the Portion at log time, so later catalog changes never retroactively alter a logged Meal. **Weight Entries** are tracked separately. A **Goal** sets daily nutrient/weight targets, each with an explicit **Direction** (target, minimum, or maximum); `get_goal_progress` and the `nom://weekly-summary` MCP resource compare logged data against the active goal.

## Installation

Build from source with the pinned Nix toolchain (recommended), or plain `cargo` if you have a suitable Rust toolchain installed (see `rust-version` in `Cargo.toml`):

```sh
nix build .#nom-mcp          # -> ./result/bin/nom-mcp
nix build .#nom-mcp-remote   # -> ./result/bin/nom-mcp-remote

# or
cargo build --release --workspace
```

## The four surfaces

`nom-mcp` exposes the same set of operations four ways, chosen by how you invoke it:

| Surface | Invocation | Use case |
|---|---|---|
| Local CLI | `nom-mcp <operation> --key value ...` | One-shot commands against the local database; no server needed. |
| MCP (stdio) | `nom-mcp serve stdio` | For MCP clients (e.g. Claude Desktop) that spawn the binary directly. |
| MCP (HTTP) + REST | `nom-mcp serve http --port 8000` | A long-running server: MCP over streamable-HTTP at `/mcp`, plus a REST API at `/api/*`. |
| Remote CLI | `nom-mcp-remote <operation> key=value ...` | The same operations as the local CLI, but takes bare `key=value` arguments (instead of the local CLI's `--key value` flags) and talks to a `serve http` server's REST API instead of opening the database directly. |

The local CLI and `nom-mcp serve` both open the SQLite database directly and hold an advisory lock while running — **do not run the local CLI against a database that a `serve` process already has open** (and vice versa); use `nom-mcp-remote` against the running server instead. See [Storage locking](#storage-locking) below.

## Usage: local CLI

Every invocation opens the local database directly, runs one operation, prints JSON to stdout, and exits.

```sh
nom-mcp <operation> [--key value ...]
```

Arguments are `--key value` long flags (`--key=value` also works); values are auto-typed by trying a JSON parse first (numbers, booleans, `null`, and `[...]`/`{...}` become JSON, everything else stays a string), so nested-JSON fields can be passed inline as single-quoted JSON. Note that `nom-mcp-remote` differs here: it takes bare `key=value` pairs instead of flags (see [Usage: `nom-mcp-remote`](#usage-nom-mcp-remote)). `nom-mcp --help` / `nom-mcp <operation> --help` prints available subcommands.

### Operations

| Operation | Purpose |
|---|---|
| `search_food --query <text or barcode>` | Search Custom Foods + USDA FDC (free text) or OpenFoodFacts (all-digit barcode). Every result is upserted into the local cache, so its `food_id` is immediately usable. |
| `create_custom_food --name <text> --serving_size <json> --nutrients <json>` | Define a Custom Food from per-serving nutrients (`serving_size.unit` must be a gram-equivalent unit). |
| `log_meal --portions <json>` | Log a meal from one or more `{food_id, quantity, quantity_mode}` portions, plus an optional raw-macro `adjustment` and `logged_at` override. Nutrients are snapshotted at log time. |
| `update_meal --meal_id <id> ...` | Partial patch to an existing meal (`portions`, if given, fully replaces the array; `adjustment`/`logged_at` patch independently). |
| `delete_meal --meal_id <id>` | Hard delete (cascades to its portions). |
| `search_meals --query <text>` | Keyword search over logged meals' food names, most-recent-first; optional date range filter. |
| `get_meals_by_date_range --start_date <date> --end_date <date>` | Meals logged within an inclusive date range. |
| `log_weight --value <number>` | Log a body-weight entry. Value is stored as-is (no unit enforcement); optional `logged_at` allows backdating. |
| `update_weight_entry --entry_id <id> ...` | Partial patch to a weight entry's value and/or timestamp. |
| `delete_weight_entry --entry_id <id>` | Hard delete. |
| `get_weight_today` | Weight entries logged today (per the resolved Clock timezone). |
| `get_weight_by_date --date <date>` | Weight entries on a specific date. |
| `get_weight_by_date_range --start_date <date> --end_date <date>` | Weight entries within an inclusive date range. |
| `set_nutrition_goals --calories <n> --calories_direction <target\|minimum\|maximum> ...` | Set or update nutrition/weight goals. Partial patch: only provided nutrients change; others carry forward from the current active goal. `*_direction` is required the first time a nutrient is set. Supports `calories`, `protein_g`, `carbs_g`, `fat_g`, `fiber_g` (each with a `*_direction`), and `target_weight` (no direction). |
| `get_goal_progress [--date <date>]` | Per-nutrient consumed-vs-target and weight-vs-target comparison for a date (defaults to today), plus the day's Fasting Window (`fasting_hours`, derived from the gap between that day's last Meal and the next Meal). |

Two additional operations, `get_widget_display` and `set_widget_display`, exist for a future widget UI and are exposed on the **MCP surface only** (not local CLI or REST) — see [AGENTS.md](AGENTS.md#one-operation-four-surfaces).

Example:

```sh
nom-mcp search_food --query almonds
nom-mcp log_meal --portions '[{"food_id":1,"quantity":150,"quantity_mode":"grams"}]'
nom-mcp create_custom_food --name "Protein Shake" \
  --serving_size '{"quantity":1,"unit":"grams"}' \
  --nutrients '{"calories":150,"protein_g":30,"carbs_g":5,"fat_g":2,"fiber_g":0}'
nom-mcp log_weight --value 181.4
nom-mcp get_weight_by_date_range --start_date 2026-08-01 --end_date 2026-08-12
nom-mcp set_nutrition_goals --calories 2200 --calories_direction target --protein_g 150 --protein_g_direction minimum
nom-mcp get_goal_progress
```

Errors print a message to stderr and exit with a category-specific code (`3` not found, `4` validation, `5` conflict, `6` external API failure, `7` storage failure — see [AGENTS.md](AGENTS.md#unified-error-taxonomy)).

## Usage: `nom-mcp serve` (MCP + HTTP)

`nom-mcp serve` runs a long-lived server instead of one-shot dispatch. It has two transports:

```sh
nom-mcp serve            # same as `nom-mcp serve stdio`
nom-mcp serve stdio      # MCP over stdio — for clients that spawn the binary directly
nom-mcp serve http                    # MCP (streamable-HTTP at /mcp) + REST API (/api/*), port 8000
nom-mcp serve http --port 9000        # same, on a custom port
```

Both transports share identical clock/registry construction, so they expose the same operations. Logs go to stderr in both modes (stdout is reserved for the MCP protocol in stdio mode). The HTTP mode listens on `http_bind_address` (see [Configuration](#configuration)) and shuts down gracefully on `SIGINT` or `SIGTERM`.

### Connecting an MCP client

For a client that spawns the binary itself (e.g. Claude Desktop), point it at the built binary with the `serve` argument, for example:

```json
{
  "mcpServers": {
    "nom": {
      "command": "/path/to/nom-mcp",
      "args": ["serve"]
    }
  }
}
```

For a client that speaks streamable-HTTP MCP, start `nom-mcp serve http --port 8000` and point the client at `http://<host>:8000/mcp`.

### MCP resource: weekly summary

Besides its tools, the MCP surface exposes one resource, `nom://weekly-summary`, which returns a rolling 7-day nutrition and weight overview (daily totals, averages vs. the active goal, weight trend, and average Fasting Window) as JSON.

### REST API

`serve http` also exposes every CLI-surfaced operation as `POST /api/{operation}`, with a JSON object whose keys are the operation's argument names (e.g. `POST /api/log_weight` with body `{"value": 181.4}`) and the same JSON response/error shape as the CLI. This is what `nom-mcp-remote` talks to.

## Usage: `nom-mcp-remote`

A thin HTTP client exposing the same operations as the local CLI (`nom-mcp-remote <operation> key=value ...`) that posts to a running `serve http` server's `/api/{operation}` endpoint instead of touching the database directly, and renders results/errors identically to the local CLI. Unlike the local CLI's `--key value` flags, it takes bare `key=value` pairs. Values that are valid JSON (numbers, booleans, `null`, arrays, objects) are sent as that JSON; anything else is sent as a string — the same rules as the local CLI, so nested-JSON arguments work too:

```sh
nom-mcp-remote search_food query=almonds
nom-mcp-remote log_weight value=181.4
nom-mcp-remote log_meal portions='[{"food_id":1,"quantity":250,"quantity_mode":"grams"}]'
```

(single-quote the JSON so your shell passes the brackets and quotes through verbatim.)

It requires `[remote].server_url` to be configured (see below) and a `nom-mcp serve http` process to be running and reachable.

## Configuration

Layered precedence: hardcoded defaults < TOML file < environment variables (env always wins).

- **Config file**: `$XDG_CONFIG_HOME/nom_mcp/config.toml` (falls back to `~/.config/nom_mcp/config.toml`).
- **Database file**: `$XDG_DATA_HOME/nom_mcp/nom.db` (falls back to `~/.local/share/nom_mcp/nom.db`), created automatically. Override with the `NOM_MCP_DB_PATH` env var (e.g. to point a dev server at a throwaway DB seeded by [`seed_data`](#local-dev-instance-with-one-command-seed-data)); the MCP session store (`mcp_sessions.db`) then lives alongside the overridden DB, isolating a dev instance's session state.
- **Env vars**: prefixed `NOM_MCP_`, e.g. `NOM_MCP_TIMEZONE`, `NOM_MCP_USDA_API_KEY`. Nested keys (like the remote-CLI's server URL) use a double underscore: `NOM_MCP_remote__server_url`.

```toml
# $XDG_CONFIG_HOME/nom_mcp/config.toml
usda_api_key = "..."              # optional — get one free at https://api.data.gov/signup
off_username = "..."              # optional — OpenFoodFacts account username (Basic auth)
off_password = "..."              # optional — OpenFoodFacts account password (Basic auth)
off_app_uuid = "..."              # optional — per-installation OFF identifier; auto-generated + persisted if unset
timezone = "America/New_York"     # optional IANA name; falls back to OS-local, then UTC
http_bind_address = "127.0.0.1"   # bind address for `nom-mcp serve http` (accepts IPv4, IPv6, or hostname)
ui_blocked_clients = ["claude-ios"]  # optional — client names whose widget `_meta.ui` pointers are suppressed (env: comma-separated NOM_MCP_UI_BLOCKED_CLIENTS)
ui_domain = "..."                 # optional — MCP Apps UI sandbox origin emitted as `_meta.ui.domain`; leave unset unless your host requires it

[remote]
server_url = "http://localhost:8000"  # only read by nom-mcp-remote; must point at a running `serve http`
```

The USDA FDC key is optional and validated lazily — `search_food` still works against Custom Foods and OpenFoodFacts without it; only a query that needs USDA data will error if the key is missing.

OpenFoodFacts credentials are also optional. When both `off_username` and `off_password` are set (or via `NOM_MCP_OFF_USERNAME` / `NOM_MCP_OFF_PASSWORD`), every OFF request carries an `Authorization: Basic ...` header — required for the OFF staging deployment (`world.openfoodfacts.net`) and any future write operations. When they're absent, nom_mcp logs a warning at startup and sends unauthenticated requests (OFF's public read API works without auth).

Every OFF request also carries the app-identity parameters OFF asks apps to send: `app_name=nom_mcp`, `app_version=<version>`, and `app_uuid`. The `app_uuid` is a stable random identifier for this installation (so OFF moderators can ban a single user without affecting the whole app account): set `off_app_uuid` in config (or `NOM_MCP_OFF_APP_UUID`) to provide your own, otherwise one is generated on first use and persisted at `$XDG_DATA_HOME/nom_mcp/off_app_uuid`.

### MCP Apps UI widgets

The `get_goal_progress` and `get_weekly_progress` tools can attach an MCP Apps `_meta.ui` pointer to their `tools/list` declarations (gated on the MCP-only `widget_display_enabled` setting) so capable hosts render an interactive widget instead of plain text. Two deployment-specific config knobs shape that behavior:

- **`ui_blocked_clients`** — client names (the `clientInfo.name` reported during MCP `initialize`, matched case-insensitively) for which the `_meta.ui` pointers are suppressed, so those clients get plain text results instead of attempting to load UI they cannot render. As of v0.5.5, Claude iOS's widget renderer fails with "Unable to connect to server" despite fully successful server handshakes, so production sets this to `claude-ios` until Anthropic fixes the client side. Stateless requests carry no identity and are never blocked.
- **`ui_domain`** — optional sandbox-origin pinning (`_meta.ui.domain`). Leave unset unless your host specifically requires it: a subtly wrong value has been observed to break rendering even on web.

## Storage locking

`nom-core` uses a non-blocking advisory-lock probe before opening the SQLite database, so running the local CLI and `nom-mcp serve` against the same database file at the same time fails fast with a clear "server is running — stop it or use the remote-CLI instead" error rather than risking silent WAL corruption from two writers. If you're running `nom-mcp serve`, use `nom-mcp-remote` for ad hoc commands instead of the local CLI. See [AGENTS.md](AGENTS.md#storage-turso--advisory-lock-handoff).

## Local dev instance with one-command seed data

Widget work (or any e2e validation of the UI) needs a local instance with realistic multi-day data. `seed_data` (local CLI only) creates or resets a throwaway database populated with a deterministic fixture: active goals covering all five nutrients plus a target weight (each with a direction), seven custom foods, eighteen meals across seven days including today (varied timing so Fasting Windows exist), seven weight entries, and widget display enabled. Values are chosen so `get_goal_progress` spans `under`, `met`, and `over` statuses, so every widget renders non-trivial content. Dates are relative to today, so coverage holds whenever it runs.

```sh
# 1. Seed a throwaway DB (never touches your real data; re-running resets it)
nom-mcp seed_data --path /tmp/nom-dev/nom.db

# 2. Start the server against it (NOM_MCP_DB_PATH points every surface at the seeded DB)
NOM_MCP_DB_PATH=/tmp/nom-dev/nom.db nom-mcp serve http --port 8000

# 3. Point a streamable-HTTP MCP client at http://localhost:8000/mcp
#    (REST: POST /api/{operation}; remote CLI:
#     NOM_MCP_remote__server_url=http://localhost:8000 nom-mcp-remote get_goal_progress)

rm -rf /tmp/nom-dev   # cleanup — the seeded DB is disposable
```

Notes:

- `seed_data` refuses to target the default database path (`$XDG_DATA_HOME/nom_mcp/nom.db`) with a validation error if you try.
- While the server holds the seeded DB open, the local CLI against that same path fails fast with the usual [storage-locking](#storage-locking) error — use `nom-mcp-remote` instead.
- Re-running `seed_data` against the same path deletes the DB (and its WAL sidecars) and re-inserts the identical rows, resetting to the same clean known state. A re-run that crosses midnight shifts dates by one day, by design.

## Development

See [AGENTS.md](AGENTS.md) for build/test/lint commands and a deep dive into the architecture (the `Operation` trait, storage locking invariants, config/Clock resolution). Quick start:

```sh
nix develop            # full dev shell (toolchain + cargo-nextest, cargo-watch, rust-analyzer, ...)
cargo build --workspace
cargo test --all-features --workspace
cargo fmt --all
cargo clippy --all-targets --all-features --workspace -- -D warnings
```

This repo uses [Backlog.md](https://github.com/MrLesk/Backlog.md) for task tracking (`backlog/tasks/`, with some older completed tasks moved to `backlog/archive/tasks/`) — design decisions live in `backlog/decisions/`, and research plus the v1 implementation spec live in `backlog/docs/`.

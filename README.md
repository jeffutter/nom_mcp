# nom_mcp

A single-user Rust nutrition tracker for logging meals, body weight, and nutrition goals — backed by OpenFoodFacts and USDA FoodData Central for food data, with local-file SQLite (via [turso](https://github.com/tursodatabase/turso)) storage. Designed to be exposed identically over MCP, a local CLI, HTTP, and a remote-CLI thin client, all driven by one shared `Operation` abstraction (see [AGENTS.md](AGENTS.md) for the architecture).

**Status: pre-v1, under active development.** The local CLI works today for food/meal/weight tracking. Goals, the Weekly Summary MCP resource, Widget Display, and the `serve` entrypoint (HTTP + MCP transports) are designed (see `backlog/docs/doc-5 - nom_mcp-v1-implementation-spec.md`) but not all wired up yet — see [Current limitations](#current-limitations) below.

## Domain model

The full glossary lives in [CONTEXT.md](CONTEXT.md). In short: a **Meal** is composed of **Portions**, each a quantity of a catalog **Food** (OpenFoodFacts barcode lookup, USDA FDC whole foods, or a user-defined Custom Food) — nutrients are snapshotted onto the Portion at log time, so later catalog changes never retroactively alter a logged Meal. **Weight Entries** are tracked separately. A **Goal** sets daily nutrient/weight targets, each with an explicit **Direction** (target, minimum, or maximum).

## Installation

Build from source with the pinned Nix toolchain (recommended), or plain `cargo` if you have a suitable Rust toolchain installed (see `rust-version` in `Cargo.toml`):

```sh
nix build .#nom-mcp          # -> ./result/bin/nom-mcp
nix build .#nom-mcp-remote   # -> ./result/bin/nom-mcp-remote

# or
cargo build --release --workspace
```

## Usage

Today, `nom-mcp` runs as a **local CLI**: every invocation opens the local database directly, runs one operation, prints JSON to stdout, and exits. There is no long-running server mode wired up yet (see [Current limitations](#current-limitations)).

```sh
nom-mcp <operation> [key=value ...]
```

Arguments are `key=value` pairs; values are auto-typed (bare numbers become JSON numbers, `true`/`false` become booleans, anything that parses as JSON — including `[...]`/`{...}` — is parsed as JSON, everything else is a string). `nom-mcp --help` / `nom-mcp <operation> --help` prints available subcommands.

Currently implemented operations:

| Operation | Purpose |
|---|---|
| `search_food query=<text or barcode>` | Search Custom Foods + USDA FDC (free text) or OpenFoodFacts (all-digit barcode). Every result is upserted into the local cache, so its `food_id` is immediately usable. |
| `create_custom_food` | Define a Custom Food from per-serving nutrients. |
| `log_meal portions=<json>` | Log a meal from one or more `{food_id, quantity, quantity_mode}` portions, plus an optional raw-macro `adjustment` and `logged_at` override. |
| `update_meal meal_id=<id> ...` | Partial patch to an existing meal (`portions`, if given, replaces the whole array). |
| `delete_meal meal_id=<id>` | Hard delete (cascades to its portions). |
| `search_meals query=<text>` | Keyword search over logged meals' food names. |
| `get_meals_by_date_range start=<date> end=<date>` | Meals logged within an inclusive date range. |
| `log_weight value=<number>` | Log a body-weight entry. |
| `update_weight_entry id=<id> ...` | Partial patch to a weight entry. |
| `delete_weight_entry id=<id>` | Hard delete. |
| `get_weight_today` | Weight entry logged today (per the resolved Clock timezone). |
| `get_weight_by_date date=<date>` | Weight entry on a specific date. |
| `get_weight_by_date_range start=<date> end=<date>` | Weight entries within an inclusive date range. |

Example:

```sh
nom-mcp search_food query=almonds
nom-mcp log_meal portions='[{"food_id":1,"quantity":150,"quantity_mode":"grams"}]'
nom-mcp log_weight value=181.4
nom-mcp get_weight_by_date_range start=2026-08-01 end=2026-08-12
```

Errors print a message to stderr and exit with a category-specific code (`3` not found, `4` validation, `5` conflict, `6` external API failure, `7` storage failure — see [AGENTS.md](AGENTS.md#unified-error-taxonomy)).

### Configuration

Layered precedence: hardcoded defaults < TOML file < environment variables (env always wins).

- **Config file**: `$XDG_CONFIG_HOME/nom_mcp/config.toml` (falls back to `~/.config/nom_mcp/config.toml`).
- **Database file**: `$XDG_DATA_HOME/nom_mcp/nom.db` (falls back to `~/.local/share/nom_mcp/nom.db`), created automatically.
- **Env vars**: prefixed `NOM_MCP_`, e.g. `NOM_MCP_TIMEZONE`, `NOM_MCP_USDA_API_KEY`. Nested keys (like the remote-CLI's server URL) use a double underscore: `NOM_MCP_remote__server_url`.

```toml
# $XDG_CONFIG_HOME/nom_mcp/config.toml
usda_api_key = "..."             # optional — get one free at https://api.data.gov/signup
timezone = "America/New_York"     # optional IANA name; falls back to OS-local, then UTC
http_bind_address = "127.0.0.1"   # for the future HTTP/MCP server mode

[remote]
server_url = "http://localhost:PORT"  # only read by nom-mcp-remote
```

The USDA FDC key is optional and validated lazily — `search_food` still works against Custom Foods and OpenFoodFacts without it; only a query that needs USDA data will error if the key is missing.

### `nom-mcp-remote`

A thin HTTP client with the same CLI surface (`nom-mcp-remote <operation> key=value ...`) that posts to a running server's `/api/{operation}` endpoint instead of touching the database directly, and renders results/errors identically to the local CLI. It requires `[remote].server_url` to be configured, and requires a server to actually be listening — see [Current limitations](#current-limitations).

### Current limitations

The `Operation`/registry architecture (see [AGENTS.md](AGENTS.md)) already supports HTTP and MCP surfaces at the library level (`nom_core::operation::{http_router, mcp_handler}`), but the `nom-mcp` binary does not yet bind an HTTP listener or start an MCP transport — only local-CLI dispatch is wired into `main()`. Until a `serve` mode lands, `nom-mcp-remote` has nothing to talk to, and there's no MCP server to point an MCP client at. Goal operations (`set_nutrition_goals`, `get_nutrition_goals`, `get_goal_progress`), the `nom://weekly-summary` MCP resource, and Widget Display are speced in `backlog/docs/doc-5 - nom_mcp-v1-implementation-spec.md` §5–8 but not yet implemented. Track progress via the Backlog.md tasks in `backlog/tasks/`.

## Development

See [AGENTS.md](AGENTS.md) for build/test/lint commands and a deep dive into the architecture (the `Operation` trait, storage locking invariants, config/Clock resolution). Quick start:

```sh
nix develop            # full dev shell (toolchain + cargo-nextest, cargo-watch, rust-analyzer, ...)
cargo build --workspace
cargo test --all-features --workspace
cargo fmt --all
cargo clippy --all-targets --all-features --workspace -- -D warnings
```

This repo uses [Backlog.md](https://github.com/MrLesk/Backlog.md) for task tracking (`backlog/`) — design decisions live in `backlog/decisions/`, research and the v1 implementation spec live in `backlog/docs/`.

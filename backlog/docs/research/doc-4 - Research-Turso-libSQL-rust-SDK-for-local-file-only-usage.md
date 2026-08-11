---
id: doc-4
title: 'Research: Turso/libSQL rust SDK for local-file-only usage'
type: other
created_date: '2026-08-11 04:47'
updated_date: '2026-08-11 04:47'
---
# Research: `libsql` Rust SDK for local-file-only usage

## 0. Important disambiguation found during research (read this first)

Turso (the company) currently ships **two different Rust crates** and the docs.turso.tech
site now documents both, which is a likely source of confusion for anyone reading the
quickstart today (Aug 2026):

- **`libsql`** (docs.rs/libsql, github.com/tursodatabase/libsql, path `libsql/` in the repo) —
  a mature, production **fork of SQLite** ("libSQL is a production-ready fork of SQLite,
  maintained by Turso... fully backwards compatible with SQLite" —
  https://docs.turso.tech/libsql). This is the crate the ticket/question is about
  (`docs.rs/libsql`).
- **`turso`** — a brand-new, **ground-up Rust rewrite** of SQLite (formerly "Limbo"),
  currently in beta, adding MVCC/concurrent writes. The current
  `https://docs.turso.tech/sdk/rust/quickstart` page now leads with *this* crate
  ("`turso` is the recommended crate for running a local database, including
  synchronizing it to and from Turso Cloud" — docs.turso.tech/sdk/rust/quickstart) and
  describes `libsql` on that same page as being for **"remote libSQL databases over
  HTTP"** — i.e. the current marketing quickstart undersells that `libsql` also has a
  fully offline local-file mode.
- Despite that framing, `docs.rs/libsql`'s own API docs are unambiguous: `libsql`'s
  `Builder::new_local()` **is** a first-class, fully offline, no-networking constructor
  (feature-gated behind the crate's default `core` feature) — see §1. It is not
  deprecated; it's just not the crate Turso is currently pushing hardest in its own
  quickstart copy.

**Recommendation for this project:** since the goal is "Turso's client library, purely
local file, no cloud account" with a stable/mature dependency, `libsql` (the SQLite fork)
is the correct choice, not the newer `turso` crate — `turso` is explicitly still in beta
and its own docs describe non-trivial current limitations around multi-process file access
(see §5). `libsql`'s local mode is just SQLite underneath, with decades of concurrency
semantics behind it.

---

## 1. Connection/builder API for a plain local file

`libsql::Builder::new_local(path)` builds a purely local database — "Creates a local
database without networking or remote connections. This variant performs no syncing and
operates entirely offline on the specified path." (docs.rs/libsql `Builder` docs,
https://docs.rs/libsql/latest/libsql/struct.Builder.html). It's gated behind the crate's
`core` feature, which is enabled by default.

Real example from the crate's own `examples/example.rs`
(https://github.com/tursodatabase/libsql/blob/main/libsql/examples/example.rs):

```rust
use libsql::Builder;

#[tokio::main]
async fn main() {
    let db = if let Ok(url) = std::env::var("LIBSQL_URL") {
        let token = std::env::var("LIBSQL_AUTH_TOKEN").unwrap_or_else(|_| {
            println!("LIBSQL_TOKEN not set, using empty token...");
            "".to_string()
        });
        Builder::new_remote(url, token).build().await.unwrap()
    } else {
        Builder::new_local(":memory:").build().await.unwrap()
    };

    let conn = db.connect().unwrap();

    conn.query("select 1; select 1;", ()).await.unwrap();

    conn.execute("CREATE TABLE IF NOT EXISTS users (email TEXT)", ())
        .await
        .unwrap();

    let stmt = conn
        .prepare("INSERT INTO users (email) VALUES (?1)")
        .await
        .unwrap();

    stmt.execute(["foo@example.com"]).await.unwrap();

    let stmt = conn
        .prepare("SELECT * FROM users WHERE email = ?1")
        .await
        .unwrap();

    let mut rows = stmt.query(["foo@example.com"]).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let value = row.get_value(0).unwrap();
    println!("Row: {:?}", value);
}
```

For a real file instead of `:memory:`, swap in a path: `Builder::new_local("app.db")`.
(This exact form also appears in the newer `turso` quickstart as
`Builder::new_local("app.db").build().await?` —
https://docs.turso.tech/sdk/rust/quickstart — the local-open call shape is essentially
identical between the two crates.)

Full set of `Builder` constructors, for context on how `new_local` fits among the others
(https://docs.rs/libsql/latest/libsql/struct.Builder.html):

| Constructor | Feature | What it does |
|---|---|---|
| `new_local(path)` | `core` (default) | Plain local file/`:memory:`, no networking, no sync |
| `new_remote(url, token)` | `remote` (default) | No local storage; all queries over HTTP to a remote libSQL/Turso server |
| `new_remote_replica(path, url, token)` | `replication` (default) | Local file that auto-syncs from/delegates writes to a remote primary — this is the "embedded replica" mode the ticket explicitly wants to avoid |
| `new_local_replica(path)` | `replication` (default) | Like remote replica but sync is manual (`Database::sync_frames`) instead of automatic |
| `new_synced_database(path, url, token)` | `sync` | Offline-first local writes that can later be pushed to a remote |

Only `new_local` is relevant to this project; the others all require Turso Cloud
credentials/URLs and should simply not be used.

## 2. Schema migration story: shipped tooling, or BYO?

**Nothing first-party.** Neither docs.turso.tech nor the `libsql` crate itself ships a
migration tool:

- The `libsql` crate's quickstart/reference pages (docs.turso.tech/sdk/rust/quickstart,
  docs.turso.tech/sdk/rust/reference) contain no mention of "migration" at all — verified
  directly by fetching and searching the quickstart page text.
- A site search of docs.turso.tech for migrations turns up only: Turso Cloud's
  **Multi-DB Schemas** feature (server-side, shared-schema propagation across cloud
  child databases — and it's explicitly *deprecated for new users*,
  https://docs.turso.tech/features/multi-db-schemas) and ORM-specific migration guides
  for other languages (Drizzle/JS, ActiveRecord/Ruby, Laravel/PHP) — none of which apply
  to a local-only Rust project.
- Practically, schema management is **plain SQL via the client**: run
  `conn.execute("CREATE TABLE ...", ())` / `ALTER TABLE` yourself, exactly as with
  `rusqlite`.
- If you want a migration *framework* rather than hand-rolled `CREATE TABLE IF NOT
  EXISTS`, the ecosystem's answer is third-party, not from Turso:
  - [`libsql_migration`](https://docs.rs/libsql_migration) (https://crates.io/crates/libsql_migration) — directory-, content-, or
    remote-URL-based `.sql` migrations; creates a `libsql_migrations` tracking table on
    first run.
  - [`geni`](https://crates.io/crates/geni) — a standalone CLI migration tool, built
    specifically because `dbmate` didn't support libSQL.
  - `refinery` (https://github.com/rust-db/refinery) does **not** support `libsql`
    directly today — its supported backends are postgres, mysql, `rusqlite`, and
    tiberius. If migration ergonomics on par with `refinery`/`rusqlite_migration` matter,
    that's a real gap versus the `rusqlite` ecosystem, since those crates support
    `rusqlite` connections natively.

**Bottom line:** schema management is fully BYO raw SQL (identical situation to
`rusqlite`); there is no Turso-maintained migration tool for the Rust `libsql` crate.

## 3. Transaction API

Obtained from a `Connection` (https://docs.rs/libsql/latest/libsql/struct.Transaction.html):

- `conn.transaction().await` — begins a transaction in `DEFERRED` mode (the default).
- `conn.transaction_with_behavior(TransactionBehavior).await` — explicit behavior
  (`Deferred`, `Immediate`, `Exclusive`, `ReadOnly` — "sqlite3 transactions and
  additional ones introduced by libsql").
- `Transaction` implements `Deref<Target = Connection>`, so all normal `execute`/
  `query`/`prepare` calls work directly on the transaction handle.
- `tx.commit().await` — "Consume this transaction and commit."
- `tx.rollback().await` — "Consume this transaction and rollback."
- `Transaction` implements `Drop`: if neither `commit()` nor `rollback()` is called, it
  rolls back automatically when dropped.

Real example from `examples/transaction.rs`
(https://github.com/tursodatabase/libsql/blob/main/libsql/examples/transaction.rs):

```rust
let tx = conn
    .transaction_with_behavior(libsql::TransactionBehavior::Immediate)
    .await
    .unwrap();

tx.execute("INSERT INTO foo (x) VALUES (?1)", ["hello world"])
    .await
    .unwrap();

tx.commit().await.unwrap();
```

Prepared statements (docs.turso.tech/sdk/rust/reference, and `examples/example.rs`):

```rust
let stmt = conn.prepare("SELECT * FROM users WHERE id = ?1").await?;
let row = stmt.query([1]).await?.next().await?.unwrap();
```

Positional (`?1`) and named (`:name` via `libsql::named_params!`) parameters are both
supported. Statements returned by `prepare()` are documented as "cached prepared
statements" (docs.rs/libsql crate docs).

## 4. Comparison to plain `rusqlite`

`libsql` is, in local-file mode, essentially SQLite with the same on-disk format and
semantics — "libSQL is a production-ready fork of SQLite... fully backwards compatible
with SQLite" (https://docs.turso.tech/libsql) — wrapped in an **async** (`tokio`-based)
API, versus `rusqlite`'s synchronous/blocking API. The meaningful differences for this
project:

- **API shape:** every `libsql` call (`execute`, `query`, `prepare`, transaction
  methods) is `async fn` and must be awaited inside a `tokio` runtime
  (`#[tokio::main]` in every example). `rusqlite` is synchronous — no runtime needed,
  typically used with `spawn_blocking` if called from async code. If the rest of the
  server is already async (e.g. an `rmcp`/Tokio-based MCP server), `libsql`'s native
  async fits naturally without wrapping blocking calls; if not, it adds a runtime
  dependency you wouldn't otherwise need.
- **Dependency footprint:** `cargo add libsql` pulls in **`remote` and `replication` as
  default features** (hyper, tonic, tower, tokio, etc. — a full HTTP/gRPC networking
  stack), even if only `Builder::new_local` is ever called
  (https://docs.rs/crate/libsql/latest/features). To get a lean, local-only build
  comparable to `rusqlite`'s footprint, you must opt out explicitly:
  `libsql = { version = "...", default-features = false, features = ["core"] }`.
  `rusqlite` carries no such networking baggage by default.
  - Feature list confirmed from docs.rs/crate/libsql/latest/features: default features
    are `core, remote, replication, tls, libsql-sys, hrana, parser, serde, stream`;
    `core` alone (`bitflags, bytes, futures, parking_lot, libsql-sys`) is sufficient for
    local-only use.
- **Forward-compatibility with sync, without a schema change:** because `new_local`,
  `new_remote_replica`, and `new_synced_database` all share the same `Builder`/
  `Database`/`Connection`/`Transaction` API surface and the same SQLite file format, the
  stated value proposition holds — the project could add Turso Cloud sync later by
  swapping the `Builder` constructor and adding credentials, with **no schema or query
  code changes required**. `rusqlite` has no such upgrade path baked in (you'd need to
  swap crates entirely to get cloud sync).
- **Maturity/ecosystem:** `rusqlite` is older, more widely used, has a bigger ecosystem
  (`rusqlite_migration`, `refinery` support, `r2d2`/connection-pooling adapters, etc.).
  `libsql`'s local mode is functionally equivalent SQLite underneath but has a thinner
  Rust-ecosystem of complementary crates (see migration tooling gap in §2).

**Net:** in local-file mode, `libsql` is essentially "SQLite + async wrapper + optional
future sync," not a different database. It's a reasonable choice if (a) the server is
already Tokio-async and (b) there's a real chance of wanting Turso Cloud sync later. If
neither is true, `rusqlite` is the more battle-tested, lower-dependency option for pure
local use.

## 5. Concurrency: can a CLI process and a long-running server both open the same local file?

**Yes, but with a real caveat about WAL mode not being on by default.**

- Since `libsql` local mode is a genuine SQLite fork on disk, it inherits **standard
  SQLite file-locking semantics**: WAL mode has always supported one writer + multiple
  concurrent readers across *separate OS processes* on the same file via POSIX file
  locks — this is baseline SQLite behavior, not something libSQL had to add.
- **However**, an open libSQL issue confirms **WAL is not the default journal mode for
  local `libsql` drivers**: "Currently, Turso (the platform), `libsql-server`, and
  embedded replicas use WAL mode and it is the only mode supported. However `libsql`
  drivers don't have WAL mode as default. Let's change that to use WAL everywhere."
  (https://github.com/tursodatabase/libsql/issues/1553, open as of this research).
  Practically, this means a `Builder::new_local()` database defaults to SQLite's
  classic rollback-journal mode unless you explicitly run
  `conn.execute("PRAGMA journal_mode=WAL", ()).await?` after connecting. **For a
  design with a short-lived CLI process and a long-lived server process touching the
  same file, explicitly setting `journal_mode=WAL` (and a reasonable
  `PRAGMA busy_timeout=...`) is a required step**, not a default you can rely on — this
  is the single most decision-relevant footgun found in this research.
- With `journal_mode=WAL` explicitly set, concurrent access from two separate OS
  processes (CLI binary + server binary) on the same `.db` file is exactly standard
  SQLite WAL behavior: multiple concurrent readers, one writer at a time, `SQLITE_BUSY`
  handled via `busy_timeout`. This is well-trodden, stable SQLite territory — no special
  libSQL flags needed.
- **Do not confuse this with the newer `turso` crate's "Multi-Process Access" docs**
  (https://docs.turso.tech/sql-reference/multiprocess-access), which is a *different,
  experimental* feature of the ground-up Rust-rewrite database engine, not the SQLite-
  fork `libsql` crate. That page states: "By default, a Turso database file is opened
  by a single OS process... opening the same file from a second process is rejected
  with a locking error," requiring an experimental `--experimental-multiprocess-wal` /
  `.experimental_multiprocess_wal(true)` flag, restricted to 64-bit Unix, excluded on
  network filesystems, and explicitly "experimental," with the on-disk coordination
  format "may change between releases." **This limitation applies to `turso`, not to
  `libsql`.** It reinforces the §0 recommendation to use `libsql` (mature SQLite
  semantics, multi-process-safe by design) rather than `turso` (beta, single-process by
  default) for this project's CLI + server design.

## 6. Known limitations / footguns for this specific use case

1. **WAL is not on by default** (see §5) — must explicitly `PRAGMA journal_mode=WAL`
   after every fresh `new_local` open if both a CLI process and a server process may
   touch the file concurrently. Source: https://github.com/tursodatabase/libsql/issues/1553.
2. **Default Cargo features pull in a full networking/gRPC stack** (`remote`,
   `replication`, `tls`, `hrana`) that's irrelevant to local-only use and bloats build
   time/binary size unless disabled with `default-features = false, features =
   ["core"]`. Source: https://docs.rs/crate/libsql/latest/features.
3. **No first-party migration tooling** — raw SQL or a third-party crate
   (`libsql_migration`, `geni`) is required; `refinery`/`rusqlite_migration` don't
   support `libsql` connections. See §2.
4. **Async-only API** — every call requires a Tokio runtime; adds friction if any part
   of the CLI/server design wants synchronous DB access (e.g. a trivial one-shot CLI
   subcommand now needs `#[tokio::main]` just to touch the DB).
5. **Documentation currently conflates `libsql` and `turso`** — the current
   docs.turso.tech quickstart pushes the newer, beta `turso` crate and undersells
   `libsql`'s local-file mode, describing `libsql` mainly as a remote-HTTP client. This
   is a documentation-navigation footgun, not a technical one, but worth knowing so a
   future maintainer doesn't get steered toward the beta `turso` crate (which currently
   has the single-process-by-default limitation this project specifically needs to
   avoid — see §5) when following "the Turso docs."
6. **`SQLITE_BUSY` handling is still the caller's job** — as with any SQLite-family
   driver, without an explicit `busy_timeout` PRAGMA, lock contention between the CLI
   process and the server process under load will surface as `SQLITE_BUSY` errors
   rather than being queued/retried automatically.

## Sources

- https://docs.turso.tech/sdk/rust/quickstart
- https://docs.turso.tech/sdk/rust/reference
- https://docs.turso.tech/libsql
- https://docs.turso.tech/sql-reference/multiprocess-access
- https://docs.turso.tech/features/multi-db-schemas
- https://docs.rs/libsql/latest/libsql/ (crate root)
- https://docs.rs/libsql/latest/libsql/struct.Builder.html
- https://docs.rs/libsql/latest/libsql/struct.Transaction.html
- https://docs.rs/crate/libsql/latest/features
- https://github.com/tursodatabase/libsql (repo)
- https://github.com/tursodatabase/libsql/blob/main/libsql/README.md
- https://github.com/tursodatabase/libsql/blob/main/libsql/examples/example.rs
- https://github.com/tursodatabase/libsql/blob/main/libsql/examples/transaction.rs
- https://github.com/tursodatabase/libsql/issues/1553 ("Use WAL as default everywhere")
- https://docs.rs/libsql_migration
- https://crates.io/crates/geni
- https://github.com/rust-db/refinery

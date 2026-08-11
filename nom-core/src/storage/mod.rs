//! Storage module — turso database access and migrations.
//!
//! Provides connection wrapper with FK enforcement, migration runner using
//! the geni pattern (raw SQL + version tracking), and WAL checkpoint invariant.

pub mod connection;
pub mod migration;

#[cfg(test)]
mod test;

pub use connection::{Connection, StorageError};

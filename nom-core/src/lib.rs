//! nom-core — shared library for nom_mcp.
//!
//! Provides the Operation trait, capability logic for all five domain entities
//! (Food, Meal, Portion, Weight Entry, Goal), storage access via turso, and
//! external API clients (OpenFoodFacts, USDA FDC).
//!
//! Future modules:
//! - `operation` — Operation trait and registry
//! - `food` — Food entity and search/resolution logic
//! - `meal` — Meal and Portion entities
//! - `weight` — Weight Entry entity
//! - `goal` — Goal entity and progress calculation
//! - `storage` — turso database access and migrations
//! - `client::off` — OpenFoodFacts REST client
//! - `client::usda` — USDA FDC REST client
//! - `clock` — timezone-aware Clock for "today" resolution
//! - `config` — configuration loading (TOML + env)
//! - `error` — unified ErrorData taxonomy

pub mod cli;
pub mod client;
pub mod clock;
pub mod config;
pub mod error;
pub mod fasting;
pub mod food;
pub mod goal;
pub mod logging;
pub mod meal;
pub mod operation;
pub mod seed;
pub mod storage;
pub mod weekly;
pub mod weight;
pub mod widget;

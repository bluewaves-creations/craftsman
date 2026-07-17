//! Craftsman library modules — the deterministic core behind the `craftsman`
//! binary. Library modules use `thiserror`; `anyhow` lives only in `main.rs`.

pub mod config;
pub mod ledger;
pub mod plan;
pub mod spec;
pub mod verify;

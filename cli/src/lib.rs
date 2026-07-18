//! Craftsman library modules — the deterministic core behind the `craftsman`
//! binary. Library modules use `thiserror`; `anyhow` lives only in `main.rs`.

pub mod adr;
pub mod codegen;
pub mod config;
pub mod docs;
pub mod doctor;
pub mod gates;
pub mod ledger;
pub mod plan;
pub mod session;
pub mod spec;
pub mod verify;

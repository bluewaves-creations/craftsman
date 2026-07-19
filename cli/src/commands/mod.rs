//! Per-command modules of the binary: each holds its clap `Args`/subcommand
//! definitions and the command flow, keeping `main.rs` a pure dispatch
//! table. Logic stays in the library crate; these files own only argument
//! shapes, output routing, and exit-code mapping.

pub mod bootstrap;
pub mod docs;
pub mod gate;
pub mod ledger;
pub mod plan;
pub mod session;
pub mod spec;
pub mod spec_delta;
pub mod verify;

use anyhow::Context as _;

/// Exit codes are a documented contract (design doc):
/// 0 pass · 1 verification failure · 2 usage error (clap's default) ·
/// 3 orchestrator error · 4 empty selection.
pub const EXIT_PASS: i32 = 0;
pub const EXIT_VERIFICATION_FAILURE: i32 = 1;
pub const EXIT_ORCHESTRATOR_ERROR: i32 = 3;
pub const EXIT_EMPTY_SELECTION: i32 = 4;

/// The working directory — every command resolves config from here.
pub fn cwd() -> anyhow::Result<std::path::PathBuf> {
    std::env::current_dir().context("cannot determine working directory")
}

/// Load `craftsman.toml` from the working directory (the shared preamble
/// of most commands).
pub fn load() -> anyhow::Result<craftsman::config::Loaded> {
    Ok(craftsman::config::Config::load(&cwd()?)?)
}

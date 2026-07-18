//! Per-stack runner adapters. Batch 2 shipped the rust stack (cucumber-rs);
//! Batch 4 adds python (pytest-bdd) and typescript (cucumber-js); swift/bash
//! code-gen arrive in Batch 5.

pub mod cucumber_rs;

use std::path::PathBuf;

use thiserror::Error;

use crate::verify::normalize::NormalizeError;

/// Errors shared by every runner adapter. Exit code 3 territory — a runner
/// the CLI cannot drive or read truthfully is an orchestrator failure,
/// never a pass.
#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("failed to spawn `{command}` in {dir}")]
    Spawn {
        command: String,
        dir: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not prepare results path {path}")]
    ResultsPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("`{command}` failed (exit {code}) without writing results:\n{output_tail}")]
    RunnerFailed {
        command: String,
        code: String,
        output_tail: String,
    },
    #[error("runner wrote no results to {path} — {hint}")]
    NoResults { path: PathBuf, hint: String },
    #[error("cannot read results {path}")]
    ReadResults {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Normalize(#[from] NormalizeError),
}

/// Last `lines` lines of a runner's output, for failure details.
pub(crate) fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

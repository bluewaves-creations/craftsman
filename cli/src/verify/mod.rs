//! `craftsman verify` — THE gate: run SPEC.md scenarios via the stack
//! adapter and normalize every runner's output into the schema v1 results.
//!
//! Exit-code contract (enforced by the command layer from [`Outcome`]):
//! 0 all passed · 1 any failed/undefined/ambiguous · 3 tool or config
//! error · 4 empty selection (a filter or batch matching nothing is never
//! silent success — runners exit 0 on empty matches, so craftsman counts
//! scenarios itself; ADR-002/ADR-003).

pub mod adapters;
pub mod normalize;

use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;
use thiserror::Error;

use crate::config::{Config, ConfigError};
use crate::plan::{self, PlanError};
use crate::spec::{self, SpecError};
use adapters::cucumber_rs::{self, AdapterError};
use normalize::{ScenarioResult, Status};

/// What to run: everything, one plan batch, or one scenario by exact name.
#[derive(Debug, Clone)]
pub enum Selection {
    All,
    Batch(u32),
    Scenario(String),
}

/// Errors before or while running the adapter. Exit code 3 territory.
#[derive(Debug, Error)]
pub enum VerifyError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Spec(#[from] SpecError),
    #[error(transparent)]
    Plan(#[from] PlanError),
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error(
        "no supported stack in {stacks:?} — Batch 2 supports the \"rust\" stack \
         (cucumber-rs); python/typescript arrive in Batch 4, swift/bash in Batch 5"
    )]
    UnsupportedStack { stacks: Vec<String> },
    #[error("[verify] runner {runner:?} is not supported for the rust stack — use \"cucumber-rs\"")]
    UnsupportedRunner { runner: String },
}

/// Scenario counts by status.
#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct Counts {
    pub passed: usize,
    pub skipped: usize,
    pub pending: usize,
    pub undefined: usize,
    pub ambiguous: usize,
    pub failed: usize,
}

impl Counts {
    fn tally(results: &[ScenarioResult]) -> Self {
        let mut c = Self::default();
        for r in results {
            match r.status {
                Status::Passed => c.passed += 1,
                Status::Skipped => c.skipped += 1,
                Status::Pending => c.pending += 1,
                Status::Undefined => c.undefined += 1,
                Status::Ambiguous => c.ambiguous += 1,
                Status::Failed => c.failed += 1,
            }
        }
        c
    }
}

/// The verify verdict, mapped 1:1 onto exit codes by the command layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Passed,
    Failed,
    EmptySelection,
}

/// Everything a verify run produced.
#[derive(Debug)]
pub struct Report {
    pub results: Vec<ScenarioResult>,
    pub counts: Counts,
    pub outcome: Outcome,
    /// Non-fatal drift, e.g. a plan batch naming scenarios absent from the
    /// spec (plan lint owns making that an error — Batch 3).
    pub warnings: Vec<String>,
}

impl Report {
    fn empty(warnings: Vec<String>) -> Self {
        Self {
            results: Vec::new(),
            counts: Counts::default(),
            outcome: Outcome::EmptySelection,
            warnings,
        }
    }
}

/// Run the verify gate for the project containing `cwd`.
///
/// # Errors
/// [`VerifyError`] on config/spec/plan/adapter failures — all exit code 3;
/// red scenarios are not an error but a [`Outcome::Failed`] report.
pub fn run(cwd: &Path, selection: &Selection) -> Result<Report, VerifyError> {
    let loaded = Config::load(cwd)?;
    let config = loaded.config;
    let root = loaded.root;

    if !config.project.stacks.iter().any(|s| s == "rust") {
        return Err(VerifyError::UnsupportedStack {
            stacks: config.project.stacks,
        });
    }
    let runner = config.verify.runner.as_deref().unwrap_or("cucumber-rs");
    if runner != "cucumber-rs" {
        return Err(VerifyError::UnsupportedRunner {
            runner: runner.to_owned(),
        });
    }

    let feature = spec::parse_spec(&root.join(&config.project.spec))?;
    let known: HashSet<String> = spec::inventory(&feature)
        .into_iter()
        .map(|e| e.scenario)
        .collect();

    // Resolve the selection against the spec inventory first: a filter that
    // cannot match anything is exit 4 without ever invoking the runner.
    let mut warnings = Vec::new();
    let filter: Option<Vec<String>> = match selection {
        Selection::All => {
            if known.is_empty() {
                return Ok(Report::empty(vec![format!(
                    "spec {} contains no scenarios",
                    config.project.spec
                )]));
            }
            None
        }
        Selection::Scenario(name) => {
            if !known.contains(name) {
                return Ok(Report::empty(vec![format!(
                    "no scenario named {name:?} in {}",
                    config.project.spec
                )]));
            }
            Some(vec![name.clone()])
        }
        Selection::Batch(n) => {
            let requested = plan::batch_scenarios(&root.join(&config.project.plan), *n)?;
            let (found, missing): (Vec<String>, Vec<String>) =
                requested.into_iter().partition(|s| known.contains(s));
            for name in &missing {
                warnings.push(format!(
                    "plan batch {n} lists scenario {name:?} which is not in {} — \
                     plan drift; run `craftsman plan lint` (Batch 3)",
                    config.project.spec
                ));
            }
            if found.is_empty() {
                return Ok(Report::empty(warnings));
            }
            Some(found)
        }
    };

    let project_dir = config
        .verify
        .cwd
        .as_ref()
        .map_or_else(|| root.clone(), |c| root.join(c));
    let runner_target = config.verify.runner_target.as_deref().unwrap_or("spec");

    let results = cucumber_rs::run(&project_dir, runner_target, filter.as_deref())?;

    let counts = Counts::tally(&results);
    let outcome = if results.is_empty() {
        // The runner's own view of the spec matched nothing (ADR-002: empty
        // matches exit 0 — the count is the only honest signal).
        Outcome::EmptySelection
    } else if counts.failed + counts.undefined + counts.ambiguous > 0 {
        Outcome::Failed
    } else {
        Outcome::Passed
    };

    Ok(Report {
        results,
        counts,
        outcome,
        warnings,
    })
}

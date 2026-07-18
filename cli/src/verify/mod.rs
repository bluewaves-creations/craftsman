//! `craftsman verify` — THE gate: run SPEC.md scenarios via the stack
//! adapters and normalize every runner's output into the schema v1 results.
//!
//! Every stack listed in `[project] stacks` runs (Batch 4: rust, python,
//! typescript — swift/bash code-gen arrive in Batch 5); results merge into
//! one report with per-stack sections. Unknown stack names are rejected
//! upfront, before any adapter runs.
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

use crate::config::{Config, ConfigError, VerifyStack};
use crate::plan::{self, PlanError};
use crate::spec::{self, SpecError};
use adapters::AdapterError;
use adapters::{cucumber_js, cucumber_rs, pytest_bdd};
use normalize::{ScenarioResult, Status};

/// What to run: everything, one plan batch, or one scenario by exact name.
#[derive(Debug, Clone)]
pub enum Selection {
    All,
    Batch(u32),
    Scenario(String),
}

/// Errors before or while running the adapters. Exit code 3 territory.
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
        "unknown stack {stack:?} in [project] stacks — craftsman verify \
         supports \"rust\", \"python\", and \"typescript\" (swift/bash arrive \
         in Batch 5)"
    )]
    UnknownStack { stack: String },
    #[error("[verify.{stack}] runner {runner:?} is not supported — use {supported:?}")]
    UnsupportedRunner {
        stack: &'static str,
        runner: String,
        supported: &'static str,
    },
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
    fn tally<'a>(results: impl IntoIterator<Item = &'a ScenarioResult>) -> Self {
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

/// One stack's normalized results — a section of the merged report.
#[derive(Debug, Serialize)]
pub struct StackReport {
    pub stack: String,
    pub results: Vec<ScenarioResult>,
}

/// Everything a verify run produced, merged across stacks.
#[derive(Debug)]
pub struct Report {
    pub stacks: Vec<StackReport>,
    pub counts: Counts,
    pub outcome: Outcome,
    /// Non-fatal drift, e.g. a plan batch naming scenarios absent from the
    /// spec (plan lint owns making that an error).
    pub warnings: Vec<String>,
}

impl Report {
    fn empty(warnings: Vec<String>) -> Self {
        Self {
            stacks: Vec::new(),
            counts: Counts::default(),
            outcome: Outcome::EmptySelection,
            warnings,
        }
    }

    /// All results across stacks, in stack order.
    pub fn results(&self) -> impl Iterator<Item = &ScenarioResult> {
        self.stacks.iter().flat_map(|s| s.results.iter())
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

    // Reject unknown stacks before running anything: a config typo must
    // never silently shrink the verified surface.
    for stack in &config.project.stacks {
        if !matches!(stack.as_str(), "rust" | "python" | "typescript") {
            return Err(VerifyError::UnknownStack {
                stack: stack.clone(),
            });
        }
    }

    let feature = spec::parse_spec(&root.join(&config.project.spec))?;
    let known: HashSet<String> = spec::inventory(&feature)
        .into_iter()
        .map(|e| e.scenario)
        .collect();

    // Resolve the selection against the spec inventory first: a filter that
    // cannot match anything is exit 4 without ever invoking a runner.
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
                     plan drift; run `craftsman plan lint`",
                    config.project.spec
                ));
            }
            if found.is_empty() {
                return Ok(Report::empty(warnings));
            }
            Some(found)
        }
    };

    let mut stacks = Vec::new();
    for stack in &config.project.stacks {
        let section = config.verify.stack(stack);
        let results = run_stack(stack, &root, section, filter.as_deref())?;
        stacks.push(StackReport {
            stack: stack.clone(),
            results,
        });
    }

    let counts = Counts::tally(stacks.iter().flat_map(|s| s.results.iter()));
    let total: usize = stacks.iter().map(|s| s.results.len()).sum();
    let outcome = if total == 0 {
        // Every runner's own view of the spec matched nothing (ADR-002:
        // empty matches exit 0 — the count is the only honest signal).
        Outcome::EmptySelection
    } else if counts.failed + counts.undefined + counts.ambiguous > 0 {
        Outcome::Failed
    } else {
        Outcome::Passed
    };

    Ok(Report {
        stacks,
        counts,
        outcome,
        warnings,
    })
}

/// Dispatch one stack to its adapter.
fn run_stack(
    stack: &str,
    root: &Path,
    section: Option<&VerifyStack>,
    filter: Option<&[String]>,
) -> Result<Vec<ScenarioResult>, VerifyError> {
    let project_dir = section
        .and_then(|s| s.cwd.as_ref())
        .map_or_else(|| root.to_path_buf(), |c| root.join(c));
    let check_runner = |stack: &'static str, supported: &'static str| {
        let runner = section
            .and_then(|s| s.runner.as_deref())
            .unwrap_or(supported);
        if runner == supported {
            Ok(())
        } else {
            Err(VerifyError::UnsupportedRunner {
                stack,
                runner: runner.to_owned(),
                supported,
            })
        }
    };

    match stack {
        "rust" => {
            check_runner("rust", "cucumber-rs")?;
            let runner_target = section
                .and_then(|s| s.runner_target.as_deref())
                .unwrap_or("spec");
            Ok(cucumber_rs::run(&project_dir, runner_target, filter)?)
        }
        "python" => {
            check_runner("python", "pytest-bdd")?;
            let tests_dir = section
                .and_then(|s| s.tests_dir.as_deref())
                .unwrap_or("tests");
            let artifacts = root.join(".craftsman").join("cache").join("verify");
            let run = pytest_bdd::run(&project_dir, &artifacts, tests_dir, filter, false)?;
            Ok(run.results)
        }
        "typescript" => {
            check_runner("typescript", "cucumber-js")?;
            let artifacts = root.join(".craftsman").join("cache").join("verify");
            Ok(cucumber_js::run(&project_dir, &artifacts, filter)?)
        }
        // Defensive: `run` validates stack names upfront, so this arm only
        // fires if the two lists ever drift (swift/bash arrive in Batch 5).
        other => Err(VerifyError::UnknownStack {
            stack: other.to_owned(),
        }),
    }
}

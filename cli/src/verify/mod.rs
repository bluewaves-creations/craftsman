//! `craftsman verify` — THE gate: run SPEC.md scenarios via the stack
//! adapters and normalize every runner's output into the schema v1 results.
//!
//! Every stack listed in `[project] stacks` runs (rust, python, typescript,
//! and — Batch 5 — the code-gen stacks swift and bash); results merge into
//! one report with per-stack sections. Unknown stack names are rejected
//! upfront, before any adapter runs.
//!
//! Exit-code contract (enforced by the command layer from [`Outcome`]):
//! 0 all passed · 1 any failed/undefined/ambiguous · 3 tool or config
//! error · 4 empty selection (a filter or batch matching nothing is never
//! silent success — runners exit 0 on empty matches, so craftsman counts
//! scenarios itself; ADR-002/ADR-003).

pub mod adapters;
pub mod impact;
pub mod normalize;
pub mod record;
mod selection;
mod stacks;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::{Config, ConfigError};
use crate::plan::PlanError;
use crate::spec::{self, SpecError};
use adapters::AdapterError;
use normalize::{ScenarioResult, Status};
use selection::{Resolved, resolve_selection};
use stacks::run_stack;

/// What to run: everything, one plan batch, one scenario by exact name, or
/// the scenarios a diff against a git ref can affect (the impact map).
#[derive(Debug, Clone)]
pub enum Selection {
    All,
    Batch(u32),
    Scenario(String),
    Impact(String),
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
         supports \"rust\", \"python\", \"typescript\", \"swift\", and \"bash\""
    )]
    UnknownStack { stack: String },
    #[error(transparent)]
    Gen(#[from] crate::codegen::GenError),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    validate_stacks(&config)?;

    let feature = spec::parse_spec(&root.join(&config.project.spec))?;
    let entries = spec::inventory(&feature);
    let gate = selection::NetworkGate::from_inventory(&entries);
    let names: Vec<String> = entries.into_iter().map(|e| e.scenario).collect();

    // Resolve the selection against the spec inventory first: a filter that
    // cannot match anything is exit 4 without ever invoking a runner.
    let mut warnings = Vec::new();
    let filter = match resolve_selection(selection, &config, &root, &names, &gate, &mut warnings)? {
        Resolved::Finished(report) => return Ok(report),
        Resolved::Filter(filter) => filter,
    };

    // A full run (no filter) refreshes the impact map from what each stack
    // cheaply knows (see the impact module docs).
    let full_run = filter.is_none();
    let mut stacks = Vec::new();
    let mut stack_maps: BTreeMap<String, impact::StackMap> = BTreeMap::new();
    for stack in &config.project.stacks {
        let section = config.verify.stack(stack);
        let run = run_stack(
            stack,
            &root,
            section,
            &feature,
            &config.project.spec,
            filter.as_deref(),
            full_run,
        )?;
        if let Some(map) = run.map {
            stack_maps.insert(stack.clone(), map);
        }
        stacks.push(StackReport {
            stack: stack.clone(),
            results: run.results,
        });
    }
    if full_run
        && !stack_maps.is_empty()
        && let Err(err) = impact::save(&root, &impact::ImpactMap::new(stack_maps))
    {
        warnings.push(format!(
            "could not write {} ({err}) — the next --impact run falls back to --all",
            impact::MAP_REL_PATH
        ));
    }

    Ok(assemble_report(&root, stacks, warnings))
}

/// Reject unknown stacks before running anything: a config typo must
/// never silently shrink the verified surface.
fn validate_stacks(config: &Config) -> Result<(), VerifyError> {
    for stack in &config.project.stacks {
        if !matches!(
            stack.as_str(),
            "rust" | "python" | "typescript" | "swift" | "bash"
        ) {
            return Err(VerifyError::UnknownStack {
                stack: stack.clone(),
            });
        }
    }
    Ok(())
}

/// Tally the merged results into the verdict and persist the run for
/// `spec status` (single-writer). Empty runs are never recorded — they
/// would wipe a previous run's verdicts with nothing.
fn assemble_report(root: &Path, stacks: Vec<StackReport>, warnings: Vec<String>) -> Report {
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
    let report = Report {
        stacks,
        counts,
        outcome,
        warnings,
    };
    if total > 0 {
        record::persist(root, &report);
    }
    report
}

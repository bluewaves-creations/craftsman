//! `craftsman mutate` — diff-scoped mutation testing.
//!
//! Research verdict (production-grade doc): coverage is a floor, never a
//! target — AI-generated suites reach high coverage with dismal fault
//! detection (57.3% kill rate observed; mutant survival 15–25% higher at
//! equal coverage). Mutation score is structurally harder to game, but
//! full runs are too slow for a loop gate, so this gate is **diff-scoped
//! by design**: only mutants in code touched since `HEAD` are exercised.
//! Full runs exist behind `--all --yes-slow` (clap enforces the pairing:
//! `--all` without `--yes-slow` is a usage error, exit 2 — the refusal is
//! parser-level, matching the exit-code contract's "2 usage").
//!
//! Per stack (ADR-004 records the tool decisions):
//!
//! - **rust** — cargo-mutants, installed hermetically via `cargo install
//!   --version <pin> --root ~/.craftsman/tools/cargo-mutants@<pin>`
//!   (uniform across platforms; the rust toolchain is already required).
//!   The diff feeds `--in-diff`; verdicts parse from
//!   `mutants.out/outcomes.json` (schema observed live against 27.1.0).
//!   Test args are pinned to `--lib --bins`: cargo-mutants builds in a
//!   copy of the package tree, where integration tests that read files
//!   outside the package (this repo's own SPEC.md harness) cannot run.
//! - **python** — mutmut pinned to 2.5.1: mutmut 3.x moved source-path
//!   selection into config files only (no CLI override), so its
//!   diff-scoping story is weak; 2.5.1's `--paths-to-mutate` scopes to
//!   changed files directly (file granularity — coarser than rust's
//!   line-level `--in-diff`). Runs inside the project env via
//!   `uv run --with` (house rule: python through uv). Survivors are
//!   reported per run, not per line: mutmut 2's results browser crashes
//!   on python ≥ 3.13 (pony ORM), an accepted v1 limit.
//! - **typescript** — Stryker (`bunx @stryker-mutator/core`) in
//!   incremental mode, `--mutate` scoped to changed files; verdicts from
//!   the mutation-testing-report-schema JSON.
//! - **swift / bash** — refused loudly ([`GateError::MutateUnsupported`]):
//!   no production-consensus tool exists; a stack this gate cannot
//!   exercise is never reported green.
//!
//! Verdict: mutation score (caught + timeout, over caught + timeout +
//! missed) on the changed code must reach `[mutate] min-score` (default
//! 60). Survived mutants become findings (`rule = survived-mutant`).
//! Baseline mode is not meaningful for a score threshold — the score IS
//! the ratchet — so baseline configs enforce strict with a note.

mod python;
mod rust;
mod ts;

use std::path::Path;
use std::time::Instant;

use python::python_mutate;
use rust::rust_mutate;
use ts::ts_mutate;

use super::{Finding, GateError, GateOutcome};
use crate::config::{Config, GateMode};

/// What to mutate. `All` is reachable only through `--all --yes-slow`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Mutants in code changed against `HEAD` (the default).
    Diff,
    /// Every mutant — slow; explicit consent required at the CLI.
    All,
}

/// Pinned tool versions (`[gates.tools]` keys override).
pub(super) const CARGO_MUTANTS_VERSION: &str = "27.1.0";
pub(super) const MUTMUT_VERSION: &str = "2.5.1";
pub(super) const STRYKER_VERSION: &str = "9.6.1";

/// One stack's mutation tally.
#[derive(Debug, Default)]
pub(super) struct Tally {
    caught: usize,
    missed: usize,
    timeout: usize,
    unviable: usize,
}

impl Tally {
    /// Mutation score in percent; `None` when no mutants were exercised.
    fn score(&self) -> Option<f64> {
        let killed = self.caught + self.timeout;
        let considered = killed + self.missed;
        if considered == 0 {
            return None;
        }
        #[expect(clippy::cast_precision_loss, reason = "mutant counts are tiny")]
        Some(killed as f64 / considered as f64 * 100.0)
    }
}

/// Run the mutate gate.
///
/// # Errors
/// [`GateError::MutateUnsupported`] when a configured stack has no
/// mutation tool; tool resolution/spawn/parse failures.
pub fn run(
    root: &Path,
    config: &Config,
    scope: Scope,
    mode: GateMode,
) -> Result<GateOutcome, GateError> {
    let min_score = config.mutate.min_score();
    let mut notes: Vec<String> = Vec::new();
    let mut findings: Vec<Finding> = Vec::new();
    let mut blocking: Vec<Finding> = Vec::new();
    let mut tools_ran: Vec<&'static str> = Vec::new();

    if mode == GateMode::Baseline {
        notes.push(
            "mutate: baseline mode is not meaningful for a score threshold \
             — enforcing strict (the score is the ratchet)"
                .to_owned(),
        );
    }

    for stack in &config.project.stacks {
        let cwd = config
            .verify
            .stack(stack)
            .and_then(|s| s.cwd.as_deref())
            .map(|c| c.trim_end_matches('/').to_owned());
        let started = Instant::now();
        let result = match stack.as_str() {
            "rust" => rust_mutate(root, config, cwd.as_deref(), scope)?,
            "python" => python_mutate(root, config, cwd.as_deref(), scope)?,
            "typescript" => ts_mutate(root, config, cwd.as_deref(), scope)?,
            other => {
                return Err(GateError::MutateUnsupported {
                    stack: other.to_owned(),
                });
            }
        };
        let elapsed = started.elapsed().as_secs_f64();
        let StackRun {
            tool,
            tally,
            findings: stack_findings,
            note,
        } = result;
        if let Some(note) = note {
            notes.push(note);
            continue; // nothing ran for this stack (no changes)
        }
        tools_ran.push(tool);
        match tally.score() {
            None => notes.push(format!(
                "mutate[{stack}]: no viable mutants in scope \
                 ({} unviable) in {elapsed:.0}s — nothing to score",
                tally.unviable
            )),
            Some(achieved) => {
                notes.push(format!(
                    "mutate[{stack}]: score {achieved:.1}% — {} caught + {} \
                     timeout / {} missed ({} unviable) in {elapsed:.0}s \
                     (threshold {min_score})",
                    tally.caught, tally.timeout, tally.missed, tally.unviable
                ));
                if achieved < min_score {
                    blocking.extend(stack_findings.iter().cloned());
                }
            }
        }
        findings.extend(stack_findings);
    }

    Ok(GateOutcome {
        gate: "mutate",
        mode: GateMode::Strict,
        findings,
        blocking,
        baselined: 0,
        ratchet: None,
        notes,
        tools_ran,
    })
}

/// One stack's run: either a tally + survivor findings, or a skip note.
pub(super) struct StackRun {
    tool: &'static str,
    tally: Tally,
    findings: Vec<Finding>,
    note: Option<String>,
}

impl StackRun {
    fn skipped(tool: &'static str, note: String) -> Self {
        Self {
            tool,
            tally: Tally::default(),
            findings: Vec::new(),
            note: Some(note),
        }
    }
}

pub(super) fn pinned(config: &Config, key: &str, default: &str) -> String {
    config
        .gates
        .tools
        .get(key)
        .cloned()
        .unwrap_or_else(|| default.to_owned())
}

// --------------------------------------------------------------------- rust

pub(super) const fn scope_word(scope: Scope) -> &'static str {
    match scope {
        Scope::Diff => "diff-scoped",
        Scope::All => "full run",
    }
}

/// Changed files for a stack, expressed relative to its cwd, filtered by
/// extension.
pub(super) fn changed_stack_files(
    root: &Path,
    cwd: Option<&str>,
    exts: &[&str],
) -> Result<Vec<String>, GateError> {
    Ok(filter_stack_files(super::changed_files(root)?, cwd, exts))
}

/// Tracked files for a stack, relative to its cwd, filtered by extension.
pub(super) fn tracked_stack_files(
    root: &Path,
    cwd: Option<&str>,
    exts: &[&str],
) -> Result<Vec<String>, GateError> {
    let tracked: Vec<String> = super::git(root, &["ls-files"])?
        .lines()
        .map(str::to_owned)
        .collect();
    Ok(filter_stack_files(tracked, cwd, exts))
}

pub(super) fn filter_stack_files(
    files: Vec<String>,
    cwd: Option<&str>,
    exts: &[&str],
) -> Vec<String> {
    files
        .into_iter()
        .filter_map(|f| {
            let rel = match cwd {
                Some(c) => f.strip_prefix(&format!("{c}/"))?.to_owned(),
                None => f,
            };
            Path::new(&rel)
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| exts.contains(&e))
                .then_some(rel)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tally_scores_with_timeouts_as_caught() {
        let t = Tally {
            caught: 6,
            timeout: 1,
            missed: 3,
            unviable: 2,
        };
        let score = t.score().expect("mutants ran");
        assert!((score - 70.0).abs() < 0.01, "{score}");
        assert!(Tally::default().score().is_none());
    }

    #[test]
    fn stack_files_filter_by_cwd_and_extension() {
        let files = vec![
            "cli/src/a.rs".to_owned(),
            "cli/app.py".to_owned(),
            "docs/x.py".to_owned(),
        ];
        assert_eq!(
            filter_stack_files(files.clone(), Some("cli"), &["py"]),
            vec!["app.py".to_owned()]
        );
        assert_eq!(
            filter_stack_files(files, None, &["py"]),
            vec!["cli/app.py".to_owned(), "docs/x.py".to_owned()]
        );
    }
}

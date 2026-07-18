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

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::Value;

use super::{Finding, GateError, GateOutcome, Severity, exec, tail, tools};
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
const CARGO_MUTANTS_VERSION: &str = "27.1.0";
const MUTMUT_VERSION: &str = "2.5.1";
const STRYKER_VERSION: &str = "9.6.1";

/// One stack's mutation tally.
#[derive(Debug, Default)]
struct Tally {
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
struct StackRun {
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

fn pinned(config: &Config, key: &str, default: &str) -> String {
    config
        .gates
        .tools
        .get(key)
        .cloned()
        .unwrap_or_else(|| default.to_owned())
}

// --------------------------------------------------------------------- rust

fn rust_mutate(
    root: &Path,
    config: &Config,
    cwd: Option<&str>,
    scope: Scope,
) -> Result<StackRun, GateError> {
    let version = pinned(config, "cargo-mutants", CARGO_MUTANTS_VERSION);
    let bin = resolve_cargo_mutants(&version)?;
    let dir = cwd.map_or_else(|| root.to_path_buf(), |c| root.join(c));
    let cache = root.join(".craftsman").join("cache");
    std::fs::create_dir_all(&cache).map_err(|source| GateError::Io {
        path: cache.clone(),
        source,
    })?;

    let mut argv = vec![
        bin.to_string_lossy().into_owned(),
        "mutants".to_owned(),
        "--no-shuffle".to_owned(),
        "--output".to_owned(),
        cache.join("mutants").to_string_lossy().into_owned(),
    ];
    if scope == Scope::Diff {
        let diff = stack_diff(root, cwd)?;
        if diff.trim().is_empty() {
            return Ok(StackRun::skipped(
                "cargo-mutants",
                "mutate[rust]: no tracked changes against HEAD — nothing to mutate".to_owned(),
            ));
        }
        let diff_path = cache.join("mutate-rust.diff");
        std::fs::write(&diff_path, &diff).map_err(|source| GateError::Io {
            path: diff_path.clone(),
            source,
        })?;
        argv.push("--in-diff".to_owned());
        argv.push(diff_path.to_string_lossy().into_owned());
    }
    // Unit tests only: the mutants build tree is a copy of the package,
    // where integration tests reading outside it cannot pass (module docs).
    argv.extend(["--".to_owned(), "--lib".to_owned(), "--bins".to_owned()]);

    eprintln!(
        "gate mutate: cargo-mutants@{version} ({}) …",
        scope_word(scope)
    );
    let output = exec(&argv, &dir, &[])?;
    let code = output.status.code().unwrap_or(-1);
    // Observed 27.1.0: 0 = all caught, 2 = missed, 3 = timeouts,
    // 4 = unviable; anything else is a tool failure.
    if !matches!(code, 0 | 2 | 3 | 4) {
        return Err(GateError::ToolFailed {
            tool: "cargo-mutants".to_owned(),
            code: code.to_string(),
            output: tail(
                &format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
                30,
            ),
        });
    }

    // A diff touching no mutable code (comments, docs) yields zero
    // mutants — observed live: cargo-mutants then exits 0 WITHOUT writing
    // mutants.out. Exit 0 with no report is that case, not a tool failure.
    let out_dir = cache.join("mutants").join("mutants.out");
    if code == 0 && !out_dir.join("outcomes.json").is_file() {
        return Ok(StackRun::skipped(
            "cargo-mutants",
            "mutate[rust]: the diff touches no mutable code — zero mutants \
             (cargo-mutants wrote no report)"
                .to_owned(),
        ));
    }

    let outcomes_path = cache
        .join("mutants")
        .join("mutants.out")
        .join("outcomes.json");
    let text = std::fs::read_to_string(&outcomes_path).map_err(|source| GateError::Io {
        path: outcomes_path,
        source,
    })?;
    let (tally, findings) = parse_cargo_mutants_outcomes(&text, cwd)?;
    Ok(StackRun {
        tool: "cargo-mutants",
        tally,
        findings,
        note: None,
    })
}

/// Hermetic cargo-mutants: `cargo install` into
/// `~/.craftsman/tools/cargo-mutants@<version>/` on first use.
fn resolve_cargo_mutants(version: &str) -> Result<PathBuf, GateError> {
    let dir = tools::tools_dir()?.join(format!("cargo-mutants@{version}"));
    let bin = dir.join("bin").join("cargo-mutants");
    if bin.is_file() {
        return Ok(bin);
    }
    eprintln!("tool cargo-mutants@{version}: installing via cargo install (first use) …");
    let argv: Vec<String> = [
        "cargo",
        "install",
        "cargo-mutants",
        "--version",
        version,
        "--root",
        &dir.to_string_lossy(),
        "--locked",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let output = exec(&argv, Path::new("."), &[])?;
    if !output.status.success() || !bin.is_file() {
        return Err(GateError::ToolFailed {
            tool: "cargo install cargo-mutants".to_owned(),
            code: output
                .status
                .code()
                .map_or_else(|| "signal".to_owned(), |c| c.to_string()),
            output: tail(&String::from_utf8_lossy(&output.stderr), 30),
        });
    }
    Ok(bin)
}

/// The diff against `HEAD` for one stack, path-relative to its cwd so the
/// tool (running there) can match files. Untracked new files are not in
/// the diff — they carry no baseline behavior to regress (documented).
fn stack_diff(root: &Path, cwd: Option<&str>) -> Result<String, GateError> {
    cwd.map_or_else(
        || super::git(root, &["diff", "HEAD"]),
        |c| {
            let relative = format!("--relative={c}");
            super::git(root, &["diff", "HEAD", &relative, "--", c])
        },
    )
}

/// Parse `mutants.out/outcomes.json` (schema observed live against
/// cargo-mutants 27.1.0: top-level `caught`/`missed`/`timeout`/`unviable`
/// totals; `outcomes[].scenario.Mutant.{file, span.start.line, name}` with
/// `summary == "MissedMutant"` for survivors).
fn parse_cargo_mutants_outcomes(
    text: &str,
    cwd: Option<&str>,
) -> Result<(Tally, Vec<Finding>), GateError> {
    let doc: Value = serde_json::from_str(text).map_err(|e| GateError::Parse {
        tool: "cargo-mutants",
        detail: format!("invalid outcomes.json: {e}"),
    })?;
    let count = |key: &str| {
        doc[key]
            .as_u64()
            .map(usize::try_from)
            .and_then(Result::ok)
            .ok_or_else(|| GateError::Parse {
                tool: "cargo-mutants",
                detail: format!("outcomes.json lacks the `{key}` total"),
            })
    };
    let tally = Tally {
        caught: count("caught")?,
        missed: count("missed")?,
        timeout: count("timeout")?,
        unviable: count("unviable")?,
    };
    let mut findings = Vec::new();
    for outcome in doc["outcomes"].as_array().unwrap_or(&Vec::new()) {
        if outcome["summary"] != "MissedMutant" {
            continue;
        }
        let mutant = &outcome["scenario"]["Mutant"];
        let file = mutant["file"].as_str().unwrap_or_default();
        let file = cwd.map_or_else(|| file.to_owned(), |c| format!("{c}/{file}"));
        findings.push(Finding {
            gate: "mutate",
            tool: "cargo-mutants",
            rule: "survived-mutant".to_owned(),
            file,
            line: mutant["span"]["start"]["line"].as_u64(),
            message: mutant["name"]
                .as_str()
                .unwrap_or("mutant survived the test suite")
                .to_owned(),
            severity: Severity::Medium,
        });
    }
    Ok((tally, findings))
}

// ------------------------------------------------------------------- python

fn python_mutate(
    root: &Path,
    config: &Config,
    cwd: Option<&str>,
    scope: Scope,
) -> Result<StackRun, GateError> {
    let version = pinned(config, "mutmut", MUTMUT_VERSION);
    let dir = cwd.map_or_else(|| root.to_path_buf(), |c| root.join(c));
    let paths = match scope {
        Scope::Diff => {
            let changed = changed_stack_files(root, cwd, &["py"])?;
            if changed.is_empty() {
                return Ok(StackRun::skipped(
                    "mutmut",
                    "mutate[python]: no changed .py files — nothing to mutate".to_owned(),
                ));
            }
            changed
        }
        Scope::All => {
            let all: Vec<String> = tracked_stack_files(root, cwd, &["py"])?;
            if all.is_empty() {
                return Ok(StackRun::skipped(
                    "mutmut",
                    "mutate[python]: no tracked .py files".to_owned(),
                ));
            }
            all
        }
    };
    let tests_dir = config
        .verify
        .stack("python")
        .and_then(|s| s.tests_dir.as_deref())
        .unwrap_or("tests")
        .to_owned();
    // File-granular scoping via mutmut 2.5.1's --paths-to-mutate; test
    // files themselves are excluded from mutation.
    let sources: Vec<String> = paths
        .iter()
        .filter(|p| !p.starts_with(&format!("{tests_dir}/")))
        .cloned()
        .collect();
    if sources.is_empty() {
        return Ok(StackRun::skipped(
            "mutmut",
            "mutate[python]: only test files changed — nothing to mutate".to_owned(),
        ));
    }
    let argv = vec![
        "uv".to_owned(),
        "run".to_owned(),
        "--with".to_owned(),
        format!("mutmut=={version}"),
        "mutmut".to_owned(),
        "run".to_owned(),
        "--paths-to-mutate".to_owned(),
        sources.join(","),
        "--tests-dir".to_owned(),
        tests_dir,
        "--no-progress".to_owned(),
        "--simple-output".to_owned(),
    ];
    eprintln!("gate mutate: mutmut@{version} ({}) …", scope_word(scope));
    let output = exec(&argv, &dir, &[])?;
    let code = output.status.code().unwrap_or(-1);
    // mutmut's exit code is a bitmask: 1 error, 2 survived, 4 timeout,
    // 8 suspicious. Any code with the error bit (or out of range) failed.
    if !(0..=14).contains(&code) || code & 1 == 1 {
        return Err(GateError::ToolFailed {
            tool: "mutmut".to_owned(),
            code: code.to_string(),
            output: tail(
                &format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
                30,
            ),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tally = parse_mutmut_counts(&stdout)?;
    // Aggregate survivors only: mutmut 2's per-mutant browser is broken on
    // python ≥ 3.13 (module docs) — file/line detail is a known v1 limit.
    let findings = if tally.missed > 0 {
        vec![Finding {
            gate: "mutate",
            tool: "mutmut",
            rule: "survived-mutant".to_owned(),
            file: cwd.map_or_else(|| sources.join(","), str::to_owned),
            line: None,
            message: format!(
                "{} mutant(s) survived in {} — run `mutmut results` in the \
                 stack for detail",
                tally.missed,
                sources.join(",")
            ),
            severity: Severity::Medium,
        }]
    } else {
        Vec::new()
    };
    Ok(StackRun {
        tool: "mutmut",
        tally,
        findings,
        note: None,
    })
}

/// Parse the final mutmut progress line:
/// `⠹ 3/3  KILLED 1  TIMEOUT 0  SUSPICIOUS 0  SURVIVED 2  SKIPPED 0`
/// (observed live against mutmut 2.5.1). Suspicious mutants count against
/// the score (not proven killed) but are not reported as survivors.
fn parse_mutmut_counts(stdout: &str) -> Result<Tally, GateError> {
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.contains("KILLED"))
        .ok_or_else(|| GateError::Parse {
            tool: "mutmut",
            detail: "no KILLED/SURVIVED counts in mutmut output".to_owned(),
        })?;
    let grab = |key: &str| -> Result<usize, GateError> {
        line.split_whitespace()
            .skip_while(|w| *w != key)
            .nth(1)
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| GateError::Parse {
                tool: "mutmut",
                detail: format!("cannot read {key} count from: {line}"),
            })
    };
    Ok(Tally {
        caught: grab("KILLED")?,
        timeout: grab("TIMEOUT")?,
        missed: grab("SURVIVED")? + grab("SUSPICIOUS")?,
        unviable: grab("SKIPPED")?,
    })
}

// --------------------------------------------------------------- typescript

fn ts_mutate(
    root: &Path,
    config: &Config,
    cwd: Option<&str>,
    scope: Scope,
) -> Result<StackRun, GateError> {
    let version = pinned(config, "stryker", STRYKER_VERSION);
    let dir = cwd.map_or_else(|| root.to_path_buf(), |c| root.join(c));
    let mut argv = vec![
        "bunx".to_owned(),
        format!("@stryker-mutator/core@{version}"),
        "run".to_owned(),
        "--incremental".to_owned(),
        "--reporters".to_owned(),
        "json".to_owned(),
    ];
    if scope == Scope::Diff {
        let changed = changed_stack_files(root, cwd, &["ts", "tsx", "js", "jsx"])?;
        if changed.is_empty() {
            return Ok(StackRun::skipped(
                "stryker",
                "mutate[typescript]: no changed source files — nothing to mutate".to_owned(),
            ));
        }
        argv.push("--mutate".to_owned());
        argv.push(changed.join(","));
    }
    eprintln!("gate mutate: stryker@{version} ({}) …", scope_word(scope));
    let output = exec(&argv, &dir, &[])?;
    let code = output.status.code().unwrap_or(-1);
    // Stryker exits 1 when its own break threshold trips; the report still
    // carries the verdict. Anything else is a tool failure.
    let report_path = dir.join("reports").join("mutation").join("mutation.json");
    if !matches!(code, 0 | 1) || !report_path.is_file() {
        return Err(GateError::ToolFailed {
            tool: "stryker".to_owned(),
            code: code.to_string(),
            output: tail(
                &format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
                30,
            ),
        });
    }
    let text = std::fs::read_to_string(&report_path).map_err(|source| GateError::Io {
        path: report_path,
        source,
    })?;
    let (tally, findings) = parse_stryker_report(&text, cwd)?;
    Ok(StackRun {
        tool: "stryker",
        tally,
        findings,
        note: None,
    })
}

/// Parse a mutation-testing-report-schema document (Stryker's JSON
/// reporter; shape per the official schema:
/// `files.<path>.mutants[].{status, location.start.line, mutatorName,
/// replacement}`). Killed/Timeout are caught; Survived/NoCoverage missed;
/// CompileError/Ignored excluded from the score.
fn parse_stryker_report(text: &str, cwd: Option<&str>) -> Result<(Tally, Vec<Finding>), GateError> {
    let doc: Value = serde_json::from_str(text).map_err(|e| GateError::Parse {
        tool: "stryker",
        detail: format!("invalid mutation report: {e}"),
    })?;
    let files = doc["files"].as_object().ok_or_else(|| GateError::Parse {
        tool: "stryker",
        detail: "mutation report lacks a `files` object".to_owned(),
    })?;
    let mut tally = Tally::default();
    let mut findings = Vec::new();
    for (path, entry) in files {
        let file = cwd.map_or_else(|| path.clone(), |c| format!("{c}/{path}"));
        for mutant in entry["mutants"].as_array().unwrap_or(&Vec::new()) {
            match mutant["status"].as_str().unwrap_or_default() {
                "Killed" => tally.caught += 1,
                "Timeout" => tally.timeout += 1,
                "Survived" | "NoCoverage" => {
                    tally.missed += 1;
                    findings.push(Finding {
                        gate: "mutate",
                        tool: "stryker",
                        rule: "survived-mutant".to_owned(),
                        file: file.clone(),
                        line: mutant["location"]["start"]["line"].as_u64(),
                        message: format!(
                            "{}: mutant survived ({})",
                            mutant["mutatorName"].as_str().unwrap_or("mutant"),
                            mutant["replacement"].as_str().unwrap_or("replacement")
                        ),
                        severity: Severity::Medium,
                    });
                }
                _ => tally.unviable += 1,
            }
        }
    }
    Ok((tally, findings))
}

// ------------------------------------------------------------------ helpers

const fn scope_word(scope: Scope) -> &'static str {
    match scope {
        Scope::Diff => "diff-scoped",
        Scope::All => "full run",
    }
}

/// Changed files for a stack, expressed relative to its cwd, filtered by
/// extension.
fn changed_stack_files(
    root: &Path,
    cwd: Option<&str>,
    exts: &[&str],
) -> Result<Vec<String>, GateError> {
    Ok(filter_stack_files(super::changed_files(root)?, cwd, exts))
}

/// Tracked files for a stack, relative to its cwd, filtered by extension.
fn tracked_stack_files(
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

fn filter_stack_files(files: Vec<String>, cwd: Option<&str>, exts: &[&str]) -> Vec<String> {
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
    fn cargo_mutants_outcomes_parse_totals_and_survivors() {
        // Trimmed from a live cargo-mutants 27.1.0 run (scratch crate).
        let json = r#"{
            "outcomes": [
                {"scenario": "Baseline", "summary": "Success"},
                {"scenario": {"Mutant": {"name": "src/lib.rs:20:5: replace double -> i32 with 0",
                    "file": "src/lib.rs", "span": {"start": {"line": 20, "column": 5}}}},
                 "summary": "CaughtMutant"},
                {"scenario": {"Mutant": {"name": "src/lib.rs:20:7: replace * with + in double",
                    "file": "src/lib.rs", "span": {"start": {"line": 20, "column": 7}}}},
                 "summary": "MissedMutant"}
            ],
            "total_mutants": 5, "missed": 1, "caught": 4, "timeout": 0, "unviable": 0
        }"#;
        let (tally, findings) = parse_cargo_mutants_outcomes(json, Some("cli")).expect("parses");
        assert_eq!(tally.caught, 4);
        assert_eq!(tally.missed, 1);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "cli/src/lib.rs");
        assert_eq!(findings[0].line, Some(20));
        assert_eq!(findings[0].rule, "survived-mutant");
        assert!(findings[0].message.contains("replace * with +"));

        let err = parse_cargo_mutants_outcomes("{}", None).expect_err("totals required");
        assert!(matches!(err, GateError::Parse { .. }), "{err}");
    }

    #[test]
    fn mutmut_counts_parse_from_the_progress_line() {
        // Observed live against mutmut 2.5.1 (scratch project).
        let stdout = "1. Running tests without mutations\nDone\n\n2. Checking mutants\n\u{283c} 3/3  KILLED 1  TIMEOUT 0  SUSPICIOUS 0  SURVIVED 2  SKIPPED 0\n";
        let tally = parse_mutmut_counts(stdout).expect("parses");
        assert_eq!(tally.caught, 1);
        assert_eq!(tally.missed, 2);
        assert_eq!(tally.unviable, 0);
        assert!(parse_mutmut_counts("no counts here").is_err());
    }

    #[test]
    fn stryker_report_parses_per_schema() {
        // Constructed per the mutation-testing-report-schema (Stryker's
        // documented JSON reporter format) — not captured from a live run.
        let json = r#"{
            "schemaVersion": "2",
            "thresholds": {"high": 80, "low": 60},
            "files": {
                "src/calc.ts": {
                    "language": "typescript",
                    "mutants": [
                        {"id": "1", "mutatorName": "ArithmeticOperator", "replacement": "a - b",
                         "status": "Killed", "location": {"start": {"line": 2, "column": 10}, "end": {"line": 2, "column": 15}}},
                        {"id": "2", "mutatorName": "ConditionalExpression", "replacement": "false",
                         "status": "Survived", "location": {"start": {"line": 5, "column": 3}, "end": {"line": 5, "column": 9}}},
                        {"id": "3", "mutatorName": "BlockStatement", "replacement": "{}",
                         "status": "NoCoverage", "location": {"start": {"line": 9, "column": 1}, "end": {"line": 11, "column": 2}}},
                        {"id": "4", "mutatorName": "StringLiteral", "replacement": "\"\"",
                         "status": "CompileError", "location": {"start": {"line": 12, "column": 1}, "end": {"line": 12, "column": 5}}}
                    ]
                }
            }
        }"#;
        let (tally, findings) = parse_stryker_report(json, None).expect("parses");
        assert_eq!(tally.caught, 1);
        assert_eq!(tally.missed, 2);
        assert_eq!(tally.unviable, 1);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "src/calc.ts");
        assert_eq!(findings[0].line, Some(5));
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

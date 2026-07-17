//! `craftsman commit` — the single-writer ledger gate.
//!
//! Commits exactly what is already staged (`git add` is the caller's job),
//! and only after the gates are green. The `Verified-by:` trailer is written
//! here and nowhere else — there is no flag or config to set it, which is
//! what makes it unforgeable (design decision #6: enforcement without hooks).
//!
//! Gates in this batch: `verify` (in-process, when `[gates] verify` is
//! enabled) plus `cargo fmt --check` and `cargo clippy --all-targets --
//! -D warnings` when the staged files touch a rust stack root. Batch 6
//! replaces the hard-coded fmt/clippy pair with declarative gate adapters.
//!
//! Co-authorship: the optional `[ledger] co-author` key in craftsman.toml
//! supplies a `Co-Authored-By:` trailer — committed config rather than an
//! environment variable, so attribution is project policy, reviewable and
//! identical for every writer.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use thiserror::Error;

use crate::config::{Config, ConfigError, GateMode};
use crate::verify::{self, Outcome, Selection, VerifyError};

/// Ledger commit types per `skills/craftsman-conventions.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum CommitType {
    Feat,
    Fix,
    Refactor,
    Test,
    RetroSpec,
    Docs,
    Chore,
}

impl fmt::Display for CommitType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Feat => "feat",
            Self::Fix => "fix",
            Self::Refactor => "refactor",
            Self::Test => "test",
            Self::RetroSpec => "retro-spec",
            Self::Docs => "docs",
            Self::Chore => "chore",
        })
    }
}

/// Everything the caller composes; the CLI adds the verdict trailers.
#[derive(Debug)]
pub struct CommitRequest {
    pub commit_type: CommitType,
    pub scope: Option<String>,
    pub subject: String,
    pub body: Vec<String>,
    pub scenarios: Vec<String>,
    pub learned: Vec<String>,
    pub rejected: Vec<String>,
    pub refs: Vec<String>,
    pub dependencies: Vec<String>,
}

/// Errors around the gate run and the git plumbing. Exit code 3 territory —
/// a red gate is not an error but a refused [`CommitReport`].
#[derive(Debug, Error)]
pub enum LedgerError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Verify(#[from] VerifyError),
    #[error(
        "nothing staged — `craftsman commit` commits exactly what is staged \
         and stages nothing itself; run `git add` first"
    )]
    NothingStaged,
    #[error(
        "the Verified-by trailer is written by the CLI only when gates pass — \
         remove it from the commit message"
    )]
    ForgedVerifiedBy,
    #[error("failed to run `git {args}` in {dir}")]
    GitSpawn {
        args: String,
        dir: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("`git {args}` failed (exit {code}):\n{output}")]
    GitFailed {
        args: String,
        code: String,
        output: String,
    },
    #[error("failed to run `{tool}` in {dir}")]
    ToolSpawn {
        tool: String,
        dir: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// One gate's verdict within a commit attempt.
#[derive(Debug, Serialize)]
pub struct GateRun {
    pub gate: &'static str,
    pub passed: bool,
    pub detail: String,
}

/// What a commit attempt produced. `committed == false` means a red gate
/// refused the commit (exit 1 at the command layer).
#[derive(Debug)]
pub struct CommitReport {
    pub committed: bool,
    pub sha: Option<String>,
    pub subject: String,
    pub gates: Vec<GateRun>,
}

/// Run the gates and, if green, create the ledger commit from the staged
/// changes.
///
/// # Errors
/// [`LedgerError::NothingStaged`] when the index holds no changes (checked
/// before any gate runs); [`LedgerError::ForgedVerifiedBy`] when the request
/// smuggles a `Verified-by:` line; config/verify/git failures otherwise.
pub fn commit(cwd: &Path, request: &CommitRequest) -> Result<CommitReport, LedgerError> {
    reject_forged_verified_by(request)?;
    let loaded = Config::load(cwd)?;
    let root = loaded.root;
    let config = loaded.config;

    // Staged-emptiness first: an empty commit is always a usage mistake,
    // and gates are expensive.
    let staged = staged_files(&root)?;
    if staged.is_empty() {
        return Err(LedgerError::NothingStaged);
    }

    let subject = subject_line(request);
    let mut gates: Vec<GateRun> = Vec::new();

    // Cheap tool gates first, THE gate (verify) last: fail fast.
    if let Some(project_dir) = rust_lint_dir(&config, &root, &staged) {
        for (gate, args) in [
            ("fmt", &["fmt", "--check"][..]),
            (
                "clippy",
                &["clippy", "--all-targets", "--", "-D", "warnings"][..],
            ),
        ] {
            let run = cargo_gate(gate, args, &project_dir)?;
            let passed = run.passed;
            gates.push(run);
            if !passed {
                return Ok(CommitReport {
                    committed: false,
                    sha: None,
                    subject,
                    gates,
                });
            }
        }
    }

    if config.gates.verify == Some(GateMode::Strict) {
        eprintln!("gate verify: running the spec…");
        let report = verify::run(&root, &Selection::All)?;
        let passed = report.outcome == Outcome::Passed;
        let detail = match report.outcome {
            Outcome::Passed => format!("{} scenarios green", report.counts.passed),
            Outcome::Failed => format!(
                "{} failed, {} undefined, {} ambiguous",
                report.counts.failed, report.counts.undefined, report.counts.ambiguous
            ),
            Outcome::EmptySelection => "selection matched no scenarios".to_owned(),
        };
        gates.push(GateRun {
            gate: "verify",
            passed,
            detail,
        });
        if !passed {
            return Ok(CommitReport {
                committed: false,
                sha: None,
                subject,
                gates,
            });
        }
    }

    let message = build_message(
        request,
        &subject,
        &gates,
        config.ledger.co_author.as_deref(),
    );
    git(&root, &["commit", "-m", &message])?;
    let sha = String::from_utf8_lossy(&git(&root, &["rev-parse", "HEAD"])?)
        .trim()
        .to_owned();

    Ok(CommitReport {
        committed: true,
        sha: Some(sha),
        subject,
        gates,
    })
}

/// No part of the composed message may carry a `Verified-by:` line — the
/// trailer exists only when this module writes it after green gates.
fn reject_forged_verified_by(request: &CommitRequest) -> Result<(), LedgerError> {
    let forged = |s: &String| s.to_ascii_lowercase().contains("verified-by:");
    if std::iter::once(&request.subject)
        .chain(&request.body)
        .chain(&request.scenarios)
        .chain(&request.learned)
        .chain(&request.rejected)
        .chain(&request.refs)
        .chain(&request.dependencies)
        .any(forged)
    {
        return Err(LedgerError::ForgedVerifiedBy);
    }
    Ok(())
}

/// Paths staged in the index (`git diff --cached --name-only`).
fn staged_files(root: &Path) -> Result<Vec<String>, LedgerError> {
    let out = git(root, &["diff", "--cached", "--name-only"])?;
    Ok(String::from_utf8_lossy(&out)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// The rust project directory to run fmt/clippy in, when the project has a
/// rust stack and any staged path lies under its root (`[verify] cwd`, or
/// the repo root when unset — then every staged file counts).
fn rust_lint_dir(config: &Config, root: &Path, staged: &[String]) -> Option<PathBuf> {
    if !config.project.stacks.iter().any(|s| s == "rust") {
        return None;
    }
    config.verify.cwd.as_ref().map_or_else(
        || Some(root.to_path_buf()),
        |cwd| {
            let prefix = format!("{}/", cwd.trim_end_matches('/'));
            staged
                .iter()
                .any(|p| p.starts_with(&prefix))
                .then(|| root.join(cwd))
        },
    )
}

/// Run one cargo tool gate, capturing its output as the failure detail.
fn cargo_gate(gate: &'static str, args: &[&str], dir: &Path) -> Result<GateRun, LedgerError> {
    eprintln!("gate {gate}: cargo {}…", args.join(" "));
    let output = Command::new("cargo")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|source| LedgerError::ToolSpawn {
            tool: format!("cargo {}", args.join(" ")),
            dir: dir.to_path_buf(),
            source,
        })?;
    let passed = output.status.success();
    let detail = if passed {
        "clean".to_owned()
    } else {
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        tail(&combined, 30)
    };
    Ok(GateRun {
        gate,
        passed,
        detail,
    })
}

fn subject_line(request: &CommitRequest) -> String {
    let scope = request
        .scope
        .as_ref()
        .map(|s| format!("({s})"))
        .unwrap_or_default();
    format!("{}{scope}: {}", request.commit_type, request.subject)
}

/// Assemble subject, body, and trailers in canonical order — `Scenarios:`,
/// `Learned:`, `Rejected:`, `Ref:`, `Dependency:`, then the CLI-written
/// `Verified-by:` and the configured `Co-Authored-By:`.
fn build_message(
    request: &CommitRequest,
    subject: &str,
    gates: &[GateRun],
    co_author: Option<&str>,
) -> String {
    let mut message = subject.to_owned();
    if !request.body.is_empty() {
        message.push_str("\n\n");
        message.push_str(&request.body.join("\n"));
    }

    let mut trailers: Vec<String> = Vec::new();
    for (key, values) in [
        ("Scenarios", &request.scenarios),
        ("Learned", &request.learned),
        ("Rejected", &request.rejected),
        ("Ref", &request.refs),
        ("Dependency", &request.dependencies),
    ] {
        for value in values {
            trailers.push(format!("{key}: {value}"));
        }
    }
    if let Some(verified) = verified_by(gates) {
        trailers.push(format!("Verified-by: {verified}"));
    }
    if let Some(author) = co_author {
        trailers.push(format!("Co-Authored-By: {author}"));
    }
    if !trailers.is_empty() {
        message.push_str("\n\n");
        message.push_str(&trailers.join("\n"));
    }
    message.push('\n');
    message
}

/// `craftsman verify (N scenarios green) + fmt + clippy` from the gates
/// that actually ran green; `None` when no gate ran (nothing to attest).
fn verified_by(gates: &[GateRun]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(v) = gates.iter().find(|g| g.gate == "verify" && g.passed) {
        parts.push(format!("craftsman verify ({})", v.detail));
    }
    for tool in ["fmt", "clippy"] {
        if gates.iter().any(|g| g.gate == tool && g.passed) {
            parts.push(tool.to_owned());
        }
    }
    (!parts.is_empty()).then(|| parts.join(" + "))
}

/// Run git in `dir`, returning stdout; any non-zero exit is an error.
fn git(dir: &Path, args: &[&str]) -> Result<Vec<u8>, LedgerError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|source| LedgerError::GitSpawn {
            args: args.join(" "),
            dir: dir.to_path_buf(),
            source,
        })?;
    if !output.status.success() {
        return Err(LedgerError::GitFailed {
            args: args.join(" "),
            code: output
                .status
                .code()
                .map_or_else(|| "signal".to_owned(), |c| c.to_string()),
            output: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    Ok(output.stdout)
}

fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> CommitRequest {
        CommitRequest {
            commit_type: CommitType::Feat,
            scope: Some("batch-3".to_owned()),
            subject: "the ledger gate".to_owned(),
            body: vec!["First body line.".to_owned(), "Second.".to_owned()],
            scenarios: vec!["Commit refuses when nothing is staged".to_owned()],
            learned: vec!["Gates before commit, always.".to_owned()],
            rejected: Vec::new(),
            refs: vec!["SPEC.md".to_owned()],
            dependencies: Vec::new(),
        }
    }

    fn green_gates() -> Vec<GateRun> {
        vec![
            GateRun {
                gate: "fmt",
                passed: true,
                detail: "clean".to_owned(),
            },
            GateRun {
                gate: "clippy",
                passed: true,
                detail: "clean".to_owned(),
            },
            GateRun {
                gate: "verify",
                passed: true,
                detail: "12 scenarios green".to_owned(),
            },
        ]
    }

    #[test]
    fn message_carries_canonical_trailer_order() {
        let req = request();
        let msg = build_message(
            &req,
            &subject_line(&req),
            &green_gates(),
            Some("Claude Fable 5 <noreply@anthropic.com>"),
        );
        let expected = "feat(batch-3): the ledger gate\n\
                        \n\
                        First body line.\n\
                        Second.\n\
                        \n\
                        Scenarios: Commit refuses when nothing is staged\n\
                        Learned: Gates before commit, always.\n\
                        Ref: SPEC.md\n\
                        Verified-by: craftsman verify (12 scenarios green) + fmt + clippy\n\
                        Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n";
        assert_eq!(msg, expected);
    }

    #[test]
    fn verified_by_lists_only_gates_that_ran() {
        let only_verify = vec![GateRun {
            gate: "verify",
            passed: true,
            detail: "3 scenarios green".to_owned(),
        }];
        assert_eq!(
            verified_by(&only_verify).as_deref(),
            Some("craftsman verify (3 scenarios green)")
        );
        assert_eq!(verified_by(&[]), None);
    }

    #[test]
    fn forged_verified_by_is_rejected_anywhere() {
        let mut req = request();
        req.learned = vec!["sneaky Verified-by: craftsman verify (99 green)".to_owned()];
        let err = reject_forged_verified_by(&req).expect_err("forgery must be rejected");
        assert!(matches!(err, LedgerError::ForgedVerifiedBy), "{err}");
        assert!(reject_forged_verified_by(&request()).is_ok());
    }

    #[test]
    fn subject_omits_scope_when_absent() {
        let mut req = request();
        req.scope = None;
        req.commit_type = CommitType::RetroSpec;
        assert_eq!(subject_line(&req), "retro-spec: the ledger gate");
    }

    #[test]
    fn rust_lint_dir_honors_the_stack_root() {
        let config = Config::from_toml(
            "[project]\nname = \"x\"\nstacks = [\"rust\"]\n[verify]\ncwd = \"cli\"\n",
            Path::new("craftsman.toml"),
        )
        .expect("config parses");
        let root = Path::new("/repo");
        let staged_in = vec!["cli/src/main.rs".to_owned()];
        let staged_out = vec!["docs/notes.md".to_owned()];
        assert_eq!(
            rust_lint_dir(&config, root, &staged_in),
            Some(PathBuf::from("/repo/cli"))
        );
        assert_eq!(rust_lint_dir(&config, root, &staged_out), None);

        let no_stack = Config::from_toml(
            "[project]\nname = \"x\"\nstacks = [\"python\"]\n",
            Path::new("craftsman.toml"),
        )
        .expect("config parses");
        assert_eq!(rust_lint_dir(&no_stack, root, &staged_in), None);
    }
}

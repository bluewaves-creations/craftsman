//! `craftsman commit` — the single-writer ledger gate.
//!
//! Commits exactly what is already staged (`git add` is the caller's job),
//! and only after the gates are green. The `Verified-by:` trailer is written
//! here and nowhere else — there is no flag or config to set it, which is
//! what makes it unforgeable (design decision #6: enforcement without hooks).
//!
//! The gate set is whatever `craftsman check-all --changed` runs (Batch 6a
//! took over from the Batch 3 hard-coded fmt/clippy pair): every gate
//! enabled in `[gates]`, honoring modes and the gate cache.
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

use crate::config::{Config, ConfigError};
use crate::gates::GateError;
use crate::gates::check_all::{self, GateVerdict};

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
    Gate(#[from] GateError),
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
}

/// One gate's verdict within a commit attempt.
#[derive(Debug, Serialize)]
pub struct GateRun {
    pub gate: String,
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

    // The configured gate set, exactly as `craftsman check-all --changed`
    // runs it (modes, baselines, and the gate cache included).
    eprintln!("commit gate: craftsman check-all --changed …");
    let report = check_all::run(&root, &config, true)?;
    let gates: Vec<GateRun> = report
        .gates
        .iter()
        .filter(|g| g.verdict != GateVerdict::Off)
        .map(|g| GateRun {
            gate: g.gate.to_owned(),
            passed: g.verdict != GateVerdict::Red,
            detail: g.detail.clone(),
        })
        .collect();
    if !report.passed() {
        return Ok(CommitReport {
            committed: false,
            sha: None,
            subject,
            gates,
        });
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

/// `craftsman check-all --changed (verify: …; lint: …)` from the gates
/// that actually ran green; `None` when no gate ran (nothing to attest).
fn verified_by(gates: &[GateRun]) -> Option<String> {
    let parts: Vec<String> = gates
        .iter()
        .filter(|g| g.passed)
        .map(|g| format!("{}: {}", g.gate, g.detail))
        .collect();
    (!parts.is_empty()).then(|| format!("craftsman check-all --changed ({})", parts.join("; ")))
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
                gate: "verify".to_owned(),
                passed: true,
                detail: "12 scenarios green".to_owned(),
            },
            GateRun {
                gate: "lint".to_owned(),
                passed: true,
                detail: "clean (2 tool(s))".to_owned(),
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
                        Verified-by: craftsman check-all --changed (verify: 12 scenarios green; \
                        lint: clean (2 tool(s)))\n\
                        Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n";
        assert_eq!(msg, expected);
    }

    #[test]
    fn verified_by_lists_only_gates_that_ran() {
        let only_verify = vec![GateRun {
            gate: "verify".to_owned(),
            passed: true,
            detail: "3 scenarios green".to_owned(),
        }];
        assert_eq!(
            verified_by(&only_verify).as_deref(),
            Some("craftsman check-all --changed (verify: 3 scenarios green)")
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
}

//! Gate orchestration — declarative tool adapters over hermetically pinned
//! tools (design decision #5: direct declarative adapters, not a qlty/trunk
//! wrap).
//!
//! Every gate tool is described as data (`adapter::GateTool`), resolved
//! hermetically (`tools`), and its output normalized into one [`Finding`]
//! shape. Baseline mode (`baseline`) compares findings against a committed
//! snapshot; `check_all` orchestrates the enabled gates with a file-hash
//! cache.
//!
//! Contract inherited from the design doc: a tool that is missing or broken
//! is an orchestrator error (exit 3) — a gate can never pass silently on
//! tool failure. No network in the verdict path: downloads happen only at
//! tool resolution (first use).

pub mod adapter;
pub mod baseline;
pub mod check_all;
pub mod health;
pub mod lint;
pub mod security;
pub mod tools;

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::ConfigError;

/// Finding severity, ordered weakest → strongest so thresholds compare
/// with `>=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        })
    }
}

/// One normalized finding — the single shape every gate tool's output maps
/// into.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// The gate this finding belongs to (`lint`, `security`, …).
    pub gate: &'static str,
    /// The tool that produced it (`clippy`, `gitleaks`, …).
    pub tool: &'static str,
    /// The tool's rule identifier.
    pub rule: String,
    /// File path, project-root-relative where the tool allows it.
    pub file: String,
    pub line: Option<u64>,
    pub message: String,
    pub severity: Severity,
}

/// One gate's run result — findings, the blocking subset after mode
/// handling, and the notes that keep partial runs honest.
#[derive(Debug, Serialize)]
pub struct GateOutcome {
    pub gate: &'static str,
    /// The mode this run enforced.
    pub mode: crate::config::GateMode,
    /// Every current finding (baselined ones included — visibility is not
    /// enforcement).
    pub findings: Vec<Finding>,
    /// The findings that fail the gate: all enforceable findings in strict
    /// mode, only un-baselined ones in baseline mode.
    pub blocking: Vec<Finding>,
    /// How many findings the baseline swallowed.
    pub baselined: usize,
    /// Auto-ratchet note, when the snapshot shrank.
    pub ratchet: Option<String>,
    /// Skipped tools, filters applied, resolution notes.
    pub notes: Vec<String>,
    /// Tool names that actually ran.
    pub tools_ran: Vec<&'static str>,
}

impl GateOutcome {
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.blocking.is_empty()
    }

    /// One-line human verdict for summaries and ledger trailers.
    #[must_use]
    pub fn detail(&self) -> String {
        if self.passed() {
            if self.baselined > 0 {
                format!("no new findings ({} baselined)", self.baselined)
            } else {
                format!("clean ({} tool(s))", self.tools_ran.len())
            }
        } else {
            format!(
                "{} blocking finding(s){}",
                self.blocking.len(),
                if self.baselined > 0 {
                    format!(" + {} baselined", self.baselined)
                } else {
                    String::new()
                }
            )
        }
    }
}

/// Errors raised by gate orchestration. Exit code 3 territory — findings are
/// never errors, but a tool that cannot run or be parsed always is.
#[derive(Debug, Error)]
pub enum GateError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Tool(#[from] tools::ToolError),
    #[error(transparent)]
    Verify(#[from] Box<crate::verify::VerifyError>),
    #[error("failed to spawn `{tool}` in {dir}")]
    Spawn {
        tool: String,
        dir: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "`{tool}` failed (exit {code}) without a parseable verdict — a broken \
         tool is never a green gate:\n{output}"
    )]
    ToolFailed {
        tool: String,
        code: String,
        output: String,
    },
    #[error("cannot parse {tool} output: {detail}")]
    Parse { tool: &'static str, detail: String },
    #[error("git failed: {detail}")]
    Git { detail: String },
    #[error("cannot read or write {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "gate {gate:?} is not orchestrated yet (Batch 6b) — supported today: \
         verify, lint, security"
    )]
    UnsupportedGate { gate: String },
}

/// FNV-1a 64-bit — a deterministic, dependency-free content hash for
/// fingerprints and cache keys (not security-relevant; artifact integrity
/// uses sha256 via the system `shasum`).
#[must_use]
pub fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// [`fnv64`] rendered as fixed-width hex.
#[must_use]
pub fn fnv_hex(text: &str) -> String {
    format!("{:016x}", fnv64(text.as_bytes()))
}

/// Files changed against `HEAD` (worktree + index) plus untracked files —
/// the `--changed` target set, root-relative.
///
/// # Errors
/// [`GateError::Git`] when git cannot answer (not a repo, no `HEAD`).
pub fn changed_files(root: &Path) -> Result<Vec<String>, GateError> {
    let mut files: Vec<String> = Vec::new();
    for args in [
        &["diff", "--name-only", "HEAD"][..],
        &["ls-files", "--others", "--exclude-standard"][..],
    ] {
        let out = git(root, args)?;
        files.extend(out.lines().map(str::to_owned));
    }
    files.sort_unstable();
    files.dedup();
    Ok(files)
}

/// Run git in `root`, returning stdout as UTF-8; non-zero exit is an error.
pub(crate) fn git(root: &Path, args: &[&str]) -> Result<String, GateError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|source| GateError::Spawn {
            tool: format!("git {}", args.join(" ")),
            dir: root.to_path_buf(),
            source,
        })?;
    if !output.status.success() {
        return Err(GateError::Git {
            detail: format!(
                "`git {}` exited {}: {}",
                args.join(" "),
                output
                    .status
                    .code()
                    .map_or_else(|| "signal".to_owned(), |c| c.to_string()),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Spawn `argv` in `dir` with optional environment overrides, capturing
/// everything. Only spawn failure is an error here; exit-code judgment
/// belongs to the caller (per-tool success codes).
pub(crate) fn exec(
    argv: &[String],
    dir: &Path,
    envs: &[(&str, String)],
) -> Result<Output, GateError> {
    let (program, rest) = argv.split_first().expect("argv is never empty");
    let mut cmd = Command::new(program);
    cmd.args(rest).current_dir(dir);
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output().map_err(|source| GateError::Spawn {
        tool: argv.join(" "),
        dir: dir.to_path_buf(),
        source,
    })
}

/// Last `lines` lines of `text` — failure details without the flood.
#[must_use]
pub(crate) fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv64_is_deterministic_and_spreads() {
        assert_eq!(fnv64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv_hex("a"), fnv_hex("a"));
        assert_ne!(fnv_hex("a"), fnv_hex("b"));
        assert_eq!(fnv_hex("a").len(), 16);
    }

    #[test]
    fn severity_orders_weakest_to_strongest() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Info < Severity::Low);
    }

    #[test]
    fn severity_parses_lowercase() {
        let s: Severity = serde_json::from_str("\"high\"").expect("parse");
        assert_eq!(s, Severity::High);
    }
}

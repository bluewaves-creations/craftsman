//! Gate baselines — the ratchet's memory (design decision #2: hybrid).
//!
//! Native mechanisms are wrapped where they exist (`SwiftLint`
//! `--write-baseline`, Semgrep baseline commit ref); every other tool's
//! existing findings are recorded as a unified snapshot of sorted finding
//! fingerprints in `.craftsman/baselines/<gate>.json` (committed).
//!
//! Baseline-mode runs fail only on findings absent from the snapshot. When
//! the current findings shrink below the snapshot on a full (unfiltered)
//! run, the snapshot is rewritten smaller automatically and permanently —
//! Betterer's auto-ratchet — which inherently prunes fingerprints whose
//! files no longer exist (a gone file can no longer produce its finding).
//! Ratchets never run on `--changed` runs: a partial run proves nothing
//! about findings it did not look for.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

mod manage;

pub use manage::{StatusRow, flip_strict, record_lint, status};

pub use super::iso_utc_now;
use super::{Finding, GateError, fnv_hex};

/// Committed baseline directory, root-relative.
pub const DIR: &str = ".craftsman/baselines";

/// One snapshot entry: rule + file + a hash of the message. Line numbers
/// are deliberately absent — they shift under unrelated edits and would
/// make every baseline stale on contact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Fingerprint {
    pub tool: String,
    pub rule: String,
    pub file: String,
    pub hash: String,
}

/// Fingerprint of a finding (see [`Fingerprint`] for what is excluded).
#[must_use]
pub fn fingerprint(finding: &Finding) -> Fingerprint {
    Fingerprint {
        tool: finding.tool.to_owned(),
        rule: finding.rule.clone(),
        file: finding.file.clone(),
        hash: fnv_hex(&finding.message),
    }
}

/// The Semgrep native baseline: a commit ref handed to
/// `--baseline-commit`, plus the finding count recorded at baseline time
/// (Semgrep-side diffing means those findings never surface again to be
/// counted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemgrepBaseline {
    #[serde(rename = "ref")]
    pub reference: String,
    pub count: usize,
}

/// A native per-tool baseline file (SwiftLint): craftsman records where it
/// lives and how many findings it swallowed at record time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeBaseline {
    pub file: String,
    pub count: usize,
}

/// `.craftsman/baselines/<gate>.json` — one gate's recorded debt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub version: u32,
    pub gate: String,
    pub recorded_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_ratchet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semgrep: Option<SemgrepBaseline>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swiftlint: Option<NativeBaseline>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub fingerprints: BTreeSet<Fingerprint>,
}

impl Baseline {
    /// A fresh snapshot for `gate` from current findings.
    #[must_use]
    pub fn record(gate: &str, findings: &[Finding]) -> Self {
        Self {
            version: 1,
            gate: gate.to_owned(),
            recorded_at: iso_utc_now(),
            last_ratchet: None,
            semgrep: None,
            swiftlint: None,
            fingerprints: findings.iter().map(fingerprint).collect(),
        }
    }

    /// Total recorded debt: snapshot fingerprints + native counts.
    #[must_use]
    pub fn count(&self) -> usize {
        self.fingerprints.len()
            + self.semgrep.as_ref().map_or(0, |s| s.count)
            + self.swiftlint.as_ref().map_or(0, |s| s.count)
    }
}

/// Path of a gate's baseline file.
#[must_use]
pub fn path(root: &Path, gate: &str) -> PathBuf {
    root.join(DIR).join(format!("{gate}.json"))
}

/// Load a gate's baseline, `None` when never recorded.
///
/// # Errors
/// [`GateError::Parse`] on an unreadable baseline — corrupt state must
/// never degrade into "no baseline, everything is new".
pub fn load(root: &Path, gate: &str) -> Result<Option<Baseline>, GateError> {
    let path = path(root, gate);
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|source| GateError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| GateError::Parse {
            tool: "baseline",
            detail: format!("{}: {e}", path.display()),
        })
}

/// Write a gate's baseline (single-writer: only the CLI touches these).
///
/// # Errors
/// [`GateError::Io`] on write failure.
///
/// # Panics
/// Never in practice — a [`Baseline`] always serializes to JSON.
pub fn save(root: &Path, baseline: &Baseline) -> Result<(), GateError> {
    let path = path(root, &baseline.gate);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| GateError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let text = serde_json::to_string_pretty(baseline).expect("baseline serializes");
    std::fs::write(&path, format!("{text}\n")).map_err(|source| GateError::Io { path, source })
}

/// What applying a baseline to current findings produced.
#[derive(Debug)]
pub struct Applied {
    /// Findings not in the baseline — the blocking set.
    pub new_findings: Vec<Finding>,
    /// How many baseline entries (distinct fingerprints) the current
    /// findings matched. Counted in fingerprints, not findings: the
    /// snapshot cannot tell two identical findings apart, so this is the
    /// same unit `gate status` reports — one truth, both places.
    pub baselined: usize,
    /// Human note when the snapshot auto-ratcheted smaller.
    pub ratchet: Option<String>,
}

/// Split `findings` into baselined and new against the gate's snapshot,
/// auto-ratcheting on improvement when `full` (unfiltered run).
///
/// `tools_ran` guards the ratchet: only fingerprints belonging to tools
/// that actually ran in this pass may be dropped — a tool that was skipped
/// proves nothing about its recorded findings.
///
/// # Errors
/// Baseline read/write failures.
pub fn apply(
    root: &Path,
    gate: &str,
    findings: Vec<Finding>,
    tools_ran: &[&'static str],
    full: bool,
) -> Result<Applied, GateError> {
    let Some(mut base) = load(root, gate)? else {
        return Ok(Applied {
            new_findings: findings,
            baselined: 0,
            ratchet: None,
        });
    };
    let current: BTreeSet<Fingerprint> = findings.iter().map(fingerprint).collect();
    let (new_findings, matched): (Vec<Finding>, Vec<Finding>) = findings
        .into_iter()
        .partition(|f| !base.fingerprints.contains(&fingerprint(f)));

    let mut ratchet = None;
    if full {
        let (considered, untouched): (BTreeSet<Fingerprint>, BTreeSet<Fingerprint>) = base
            .fingerprints
            .iter()
            .cloned()
            .partition(|fp| tools_ran.contains(&fp.tool.as_str()));
        let retained: BTreeSet<Fingerprint> = considered.intersection(&current).cloned().collect();
        let dropped = considered.len() - retained.len();
        if dropped > 0 {
            base.fingerprints = retained.union(&untouched).cloned().collect();
            base.last_ratchet = Some(iso_utc_now());
            save(root, &base)?;
            ratchet = Some(format!(
                "gate {gate}: baseline ratcheted down by {dropped} fixed finding(s) \
                 — now {} (rewritten, permanent)",
                base.count()
            ));
        }
    }

    let matched_entries: BTreeSet<Fingerprint> = matched.iter().map(fingerprint).collect();
    Ok(Applied {
        new_findings,
        baselined: matched_entries.len(),
        ratchet,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::Severity;

    fn f(tool: &'static str, rule: &str, file: &str, message: &str) -> Finding {
        Finding {
            gate: "lint",
            tool,
            rule: rule.to_owned(),
            file: file.to_owned(),
            line: Some(3),
            message: message.to_owned(),
            severity: Severity::Medium,
        }
    }

    #[test]
    fn fingerprints_ignore_lines_but_not_messages() {
        let mut a = f("clippy", "r", "src/a.rs", "m");
        let mut b = f("clippy", "r", "src/a.rs", "m");
        b.line = Some(99);
        assert_eq!(
            fingerprint(&a),
            fingerprint(&b),
            "lines shift; fps must not"
        );
        a.message = "other".to_owned();
        assert_ne!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn apply_without_a_baseline_marks_everything_new() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let applied = apply(
            tmp.path(),
            "lint",
            vec![f("clippy", "r", "src/a.rs", "m")],
            &["clippy"],
            true,
        )
        .expect("apply");
        assert_eq!(applied.new_findings.len(), 1);
        assert_eq!(applied.baselined, 0);
        assert!(applied.ratchet.is_none());
    }

    #[test]
    fn apply_baselines_then_ratchets_on_improvement() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let recorded = vec![
            f("clippy", "r1", "src/a.rs", "m1"),
            f("clippy", "r2", "src/b.rs", "m2"),
        ];
        save(tmp.path(), &Baseline::record("lint", &recorded)).expect("save");

        // Same findings: all baselined, nothing new, no ratchet.
        let applied = apply(tmp.path(), "lint", recorded, &["clippy"], true).expect("ok");
        assert!(applied.new_findings.is_empty());
        assert_eq!(applied.baselined, 2);
        assert!(applied.ratchet.is_none());

        // One fixed, one new: new blocks, snapshot shrinks permanently.
        let now = vec![
            f("clippy", "r1", "src/a.rs", "m1"),
            f("clippy", "r9", "src/c.rs", "fresh"),
        ];
        let applied = apply(tmp.path(), "lint", now, &["clippy"], true).expect("ok");
        assert_eq!(applied.new_findings.len(), 1);
        assert_eq!(applied.new_findings[0].rule, "r9");
        assert_eq!(applied.baselined, 1);
        assert!(applied.ratchet.expect("ratcheted").contains("1 fixed"));
        let base = load(tmp.path(), "lint").expect("load").expect("exists");
        assert_eq!(base.count(), 1);
        assert!(base.last_ratchet.is_some());
    }

    #[test]
    fn partial_runs_never_ratchet_and_skipped_tools_keep_their_debt() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let recorded = vec![
            f("clippy", "r1", "src/a.rs", "m1"),
            f("ruff", "E501", "app.py", "long line"),
        ];
        save(tmp.path(), &Baseline::record("lint", &recorded)).expect("save");

        // Changed-run (full=false) with everything fixed: no ratchet.
        let applied = apply(tmp.path(), "lint", Vec::new(), &["clippy"], false).expect("ok");
        assert!(applied.ratchet.is_none());
        // Full run where only clippy ran: ruff's fingerprint survives.
        let applied = apply(tmp.path(), "lint", Vec::new(), &["clippy"], true).expect("ok");
        assert!(applied.ratchet.is_some());
        let base = load(tmp.path(), "lint").expect("load").expect("exists");
        assert_eq!(base.count(), 1);
        assert_eq!(base.fingerprints.iter().next().expect("one").tool, "ruff");
    }

    #[test]
    fn baselined_counts_distinct_fingerprints_not_findings() {
        // Two identical findings (same tool/rule/file/message — e.g. two
        // duplicated blocks in one file) collide into one fingerprint; the
        // snapshot holds one entry, so "baselined" must report 1, matching
        // what `gate status` reads from the file. This was the observed
        // 41-vs-42 drift between check-all and gate status.
        let tmp = tempfile::tempdir().expect("tempdir");
        let twin = || f("health", "duplication", "src/a.rs", "duplicated block");
        save(tmp.path(), &Baseline::record("health", &[twin(), twin()])).expect("save");
        let base = load(tmp.path(), "health").expect("load").expect("exists");
        assert_eq!(base.count(), 1, "the snapshot dedupes to one entry");

        let applied = apply(
            tmp.path(),
            "health",
            vec![twin(), twin()],
            &["health"],
            true,
        )
        .expect("apply");
        assert!(applied.new_findings.is_empty());
        assert_eq!(applied.baselined, 1, "one entry matched, not two findings");
        assert!(applied.ratchet.is_none());
    }

    #[test]
    fn count_includes_native_swiftlint_debt() {
        // gate status and the strict flip read one number; the SwiftLint
        // native baseline (Batch 9a) must be part of it.
        let mut base = Baseline::record("lint", &[f("clippy", "r", "src/a.rs", "m")]);
        base.swiftlint = Some(NativeBaseline {
            file: format!("{DIR}/swiftlint.json"),
            count: 3,
        });
        assert_eq!(base.count(), 4);
    }

    #[test]
    fn iso_utc_now_looks_like_iso() {
        let now = iso_utc_now();
        assert_eq!(now.len(), 20, "{now}");
        assert!(now.starts_with("20"), "{now}");
        assert!(now.ends_with('Z'), "{now}");
    }
}

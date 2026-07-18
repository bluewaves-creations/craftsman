//! The last-verify record — `.craftsman/cache/last-verify.json`.
//!
//! Every `craftsman verify` run persists its normalized results here
//! (single-writer: only the CLI writes it), so `spec status` can show real
//! verdicts instead of "unknown". A filtered run (`--scenario`, `--batch`,
//! `--impact`) merges per scenario into the previous record — scenarios it
//! did not run keep their recorded verdicts (GAP-R10, decided by the human
//! 2026-07-18). The merge is same-head only: when HEAD moved since the
//! previous record, the new run replaces it outright, so verdicts from
//! different HEADs never mix (the Batch 9b concern that originally ruled
//! merging out stays honored by the guard instead).

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::normalize::{ScenarioResult, Status};
use super::{Outcome, Report};

/// Root-relative record path.
pub const REL_PATH: &str = ".craftsman/cache/last-verify.json";

/// One stack's recorded results (mirrors [`super::StackReport`], owned here
/// so the report types stay serialization-only).
#[derive(Debug, Serialize, Deserialize)]
pub struct RecordedStack {
    pub stack: String,
    pub results: Vec<ScenarioResult>,
}

/// The persisted record of one verify run.
#[derive(Debug, Serialize, Deserialize)]
pub struct LastVerify {
    pub version: u32,
    /// ISO-8601 UTC instant of the run.
    pub recorded_at: String,
    /// `git rev-parse HEAD` at record time (`"unknown"` outside a repo —
    /// staleness then cannot be judged and is reported as such).
    pub head: String,
    pub outcome: Outcome,
    pub stacks: Vec<RecordedStack>,
}

impl LastVerify {
    /// The recorded status for a scenario name — the worst across stacks —
    /// or `None` when the last run did not include it.
    #[must_use]
    pub fn scenario_status(&self, scenario: &str) -> Option<Status> {
        self.stacks
            .iter()
            .flat_map(|s| s.results.iter())
            .filter(|r| r.scenario == scenario)
            .map(|r| r.status)
            .max()
    }

    /// Has HEAD moved since the record was written? `None` when either side
    /// is unknown (not a repo).
    #[must_use]
    pub fn stale(&self, root: &Path) -> Option<bool> {
        if self.head == "unknown" {
            return None;
        }
        head(root).map(|current| current != self.head)
    }
}

/// Build the record for a finished run.
#[must_use]
pub fn from_report(root: &Path, report: &Report) -> LastVerify {
    LastVerify {
        version: 1,
        recorded_at: iso_utc_now(),
        head: head(root).unwrap_or_else(|| "unknown".to_owned()),
        outcome: report.outcome,
        stacks: report
            .stacks
            .iter()
            .map(|s| RecordedStack {
                stack: s.stack.clone(),
                results: s.results.clone(),
            })
            .collect(),
    }
}

/// Persist a finished run, downgrading write failure to a stderr warning
/// (a read-only filesystem must not turn a green verify red).
pub fn persist(root: &Path, report: &Report) {
    let mut record = from_report(root, report);
    if let Some(previous) = load(root) {
        merge_previous(&mut record, previous);
    }
    if let Err(err) = save(root, &record) {
        eprintln!("warning: could not write {REL_PATH} ({err})");
    }
}

/// Fold the previous record's verdicts into `record` for every scenario
/// the new run did not include — same-head only; a moved (or unknowable)
/// HEAD means the previous verdicts describe a different tree and the new
/// run replaces them. The recorded outcome then reflects the merged set.
fn merge_previous(record: &mut LastVerify, previous: LastVerify) {
    if record.head == "unknown" || previous.head != record.head {
        return;
    }
    for prev_stack in previous.stacks {
        let Some(stack) = record
            .stacks
            .iter_mut()
            .find(|s| s.stack == prev_stack.stack)
        else {
            record.stacks.push(prev_stack);
            continue;
        };
        for result in prev_stack.results {
            if !stack.results.iter().any(|r| r.scenario == result.scenario) {
                stack.results.push(result);
            }
        }
    }
    let all_pass = record
        .stacks
        .iter()
        .flat_map(|s| &s.results)
        .all(|r| r.status == Status::Passed);
    record.outcome = if all_pass {
        Outcome::Passed
    } else {
        Outcome::Failed
    };
}

/// Persist the record (creating `.craftsman/cache/`).
///
/// # Errors
/// The underlying I/O error — callers downgrade it to a warning (a
/// read-only filesystem must not turn a green verify red).
pub fn save(root: &Path, record: &LastVerify) -> std::io::Result<()> {
    let path = root.join(REL_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(record).map_err(std::io::Error::other)?;
    std::fs::write(path, text + "\n")
}

/// Load the record, if one exists and parses. Corrupt or version-drifted
/// records read as absent — `spec status` then honestly reports unknown.
#[must_use]
pub fn load(root: &Path) -> Option<LastVerify> {
    let text = std::fs::read_to_string(root.join(REL_PATH)).ok()?;
    let record: LastVerify = serde_json::from_str(&text).ok()?;
    (record.version == 1).then_some(record)
}

/// `git rev-parse HEAD` in `root`, best-effort.
fn head(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Current instant as ISO-8601 UTC, dependency-free (same approach as the
/// baselines module; `date -u` is POSIX).
fn iso_utc_now() -> String {
    Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or_else(
            || "unknown".to_owned(),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_owned(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::{Counts, StackReport};

    fn report(status: Status) -> Report {
        Report {
            stacks: vec![StackReport {
                stack: "rust".to_owned(),
                results: vec![ScenarioResult {
                    feature: "F".to_owned(),
                    scenario: "The loop closes".to_owned(),
                    status,
                    duration_ms: Some(3),
                    failure: None,
                }],
            }],
            counts: Counts::default(),
            outcome: if status == Status::Passed {
                Outcome::Passed
            } else {
                Outcome::Failed
            },
            warnings: Vec::new(),
        }
    }

    #[test]
    fn record_round_trips_and_answers_scenario_status() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let record = from_report(tmp.path(), &report(Status::Passed));
        assert_eq!(record.head, "unknown", "not a repo");
        save(tmp.path(), &record).expect("save");
        let back = load(tmp.path()).expect("load");
        assert_eq!(back.version, 1);
        assert_eq!(back.outcome, Outcome::Passed);
        assert_eq!(
            back.scenario_status("The loop closes"),
            Some(Status::Passed)
        );
        assert_eq!(back.scenario_status("Never ran"), None);
        assert_eq!(back.stale(tmp.path()), None, "no repo, no staleness call");
    }

    #[test]
    fn worst_status_wins_across_stacks() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut record = from_report(tmp.path(), &report(Status::Passed));
        record.stacks.push(RecordedStack {
            stack: "python".to_owned(),
            results: vec![ScenarioResult {
                feature: "F".to_owned(),
                scenario: "The loop closes".to_owned(),
                status: Status::Failed,
                duration_ms: None,
                failure: Some("boom".to_owned()),
            }],
        });
        assert_eq!(
            record.scenario_status("The loop closes"),
            Some(Status::Failed)
        );
    }

    #[test]
    fn corrupt_or_missing_records_read_as_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(load(tmp.path()).is_none());
        std::fs::create_dir_all(tmp.path().join(".craftsman/cache")).expect("mkdirs");
        std::fs::write(tmp.path().join(REL_PATH), "{nope").expect("write");
        assert!(load(tmp.path()).is_none(), "corrupt reads as absent");
    }

    #[test]
    fn staleness_tracks_head_movement() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .status()
                .expect("git");
            assert!(status.success(), "git {args:?}");
        };
        run(&["init", "--quiet"]);
        std::fs::write(tmp.path().join("a.txt"), "one").expect("write");
        run(&["add", "-A"]);
        run(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "one",
        ]);
        let record = from_report(tmp.path(), &report(Status::Passed));
        assert_eq!(record.stale(tmp.path()), Some(false));
        std::fs::write(tmp.path().join("a.txt"), "two").expect("write");
        run(&["add", "-A"]);
        run(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "two",
        ]);
        assert_eq!(record.stale(tmp.path()), Some(true), "HEAD moved");
    }
}

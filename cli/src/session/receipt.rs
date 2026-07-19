//! The boundary receipt and the session distance line.
//!
//! `craftsman extract` records the head it closed at; `spec status` and
//! `craftsman commit` report how many ledger commits (commits carrying a
//! `Verified-by:` trailer) have landed since. Pure visibility by design
//! (dogfood finding 13): never a threshold, never a block — the boundary
//! stays a human judgment; the machine only makes skipping it impossible
//! to miss.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::{SESSION_DIR, SessionError};

/// The boundary receipt: the head the last `craftsman extract` closed at,
/// the anchor the session distance line counts ledger commits from.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractReceipt {
    pub head: String,
    pub recorded_at: String,
}

const RECEIPT_FILE: &str = "last-extract.json";

/// Record where this extract closed. Outside a repository there is no
/// head to record — no receipt, no error (the extract itself succeeded).
pub(super) fn write_receipt(
    dir: &Path,
    root: &Path,
    now: &str,
) -> Result<Option<String>, SessionError> {
    let Some(head) = git_head(root) else {
        return Ok(None);
    };
    let receipt = ExtractReceipt {
        head: head.clone(),
        recorded_at: now.to_owned(),
    };
    // Invariant: two plain string fields always serialize.
    let text = serde_json::to_string_pretty(&receipt).expect("receipt serializes");
    let path = dir.join(RECEIPT_FILE);
    std::fs::write(&path, text).map_err(|source| SessionError::Io { path, source })?;
    Ok(Some(head))
}

/// The session distance line — boundary observability as pure visibility.
///
/// Never a threshold, never a block: "N ledger commits since last
/// extract", where ledger commits carry a `Verified-by:` trailer, or
/// "no extract recorded" when no receipt exists. `None` when a receipt
/// exists but git cannot answer (missing binary, unknown head) — the
/// line never invents a number.
#[must_use]
pub fn distance_line(root: &Path) -> Option<String> {
    let Some(receipt) = read_receipt(root) else {
        return Some(
            "session: no extract recorded — run `craftsman extract` at the batch boundary"
                .to_owned(),
        );
    };
    let count = ledger_commits_since(root, &receipt.head)?;
    let noun = if count == 1 { "commit" } else { "commits" };
    Some(format!("session: {count} ledger {noun} since last extract"))
}

fn read_receipt(root: &Path) -> Option<ExtractReceipt> {
    let text = std::fs::read_to_string(root.join(SESSION_DIR).join(RECEIPT_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Commits between the receipt head and HEAD whose message carries a
/// `Verified-by:` trailer line.
fn ledger_commits_since(root: &Path, head: &str) -> Option<usize> {
    let range = format!("{head}..HEAD");
    let output = Command::new("git")
        .args(["rev-list", "--count", "--grep=^Verified-by: ", &range])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

fn git_head(root: &Path) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::session::{ExtractRequest, extract};

    fn config() -> Config {
        Config::from_toml(
            "[project]\nname = \"demo\"\nstacks = [\"rust\"]\n",
            Path::new("craftsman.toml"),
        )
        .expect("minimal config")
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {args:?} failed");
    }

    fn git_commit_all(dir: &Path, message: &str) {
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "--quiet", "-m", message]);
    }

    #[test]
    fn distance_line_without_receipt_says_no_extract() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let line = distance_line(tmp.path()).expect("always a line without a receipt");
        assert!(line.contains("no extract recorded"), "{line}");
    }

    #[test]
    fn extract_records_a_receipt_and_the_distance_counts_only_ledger_commits() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        git(dir, &["init", "--quiet"]);
        git(dir, &["config", "user.name", "fixture"]);
        git(dir, &["config", "user.email", "fixture@example.invalid"]);
        std::fs::write(dir.join("a.md"), "one\n").expect("write");
        git_commit_all(dir, "chore: initial");

        let report = extract(dir, &config(), &ExtractRequest::default()).expect("extract");
        let head = report.receipt_head.expect("a receipt inside a repository");
        assert_eq!(head.len(), 40, "a full sha: {head}");
        assert_eq!(
            distance_line(dir).as_deref(),
            Some("session: 0 ledger commits since last extract")
        );

        std::fs::write(dir.join("b.md"), "two\n").expect("write");
        git_commit_all(dir, "feat: work\n\nVerified-by: test gates");
        std::fs::write(dir.join("c.md"), "three\n").expect("write");
        git_commit_all(dir, "docs: prose only");
        assert_eq!(
            distance_line(dir).as_deref(),
            Some("session: 1 ledger commit since last extract"),
            "one trailer commit counts; the trailer-less one does not"
        );
    }
}

//! `craftsman extract` — compaction by extraction (compaction research).
//!
//! At batch boundaries the agent writes durable session knowledge to
//! `.craftsman/session/` so context compression loses nothing operational.
//!
//! The split of judgment: the *agent* decides what a decision, failed
//! approach, or open question is (the flag values); the *CLI* formats and
//! writes the files (single-writer). Nothing here infers content beyond
//! state that is already mechanically parseable — plan checkbox counts and
//! `git status` — per "keep it simple and mechanical".
//!
//! Layout (progressive disclosure): `index.md` (~500 tokens, regenerated
//! every extract — the post-compaction briefing), `batch-N.md` (appended
//! per-batch detail), `learnings.md` (append-only failed approaches).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use thiserror::Error;

mod receipt;
use receipt::write_receipt;
pub use receipt::{ExtractReceipt, distance_line};

use crate::config::{Config, ConfigError};
use crate::gates::baseline::iso_utc_now;

/// Errors of the session extractor. Exit 3 territory.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("cannot read or write {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("no session extract yet — run `craftsman extract` at a batch boundary first")]
    NoIndex,
}

/// What the agent judged worth extracting; the CLI formats it.
#[derive(Debug, Default)]
pub struct ExtractRequest {
    pub batch: Option<u32>,
    pub decisions: Vec<String>,
    pub failed: Vec<String>,
    pub open: Vec<String>,
}

/// What was written.
#[derive(Debug, Serialize)]
pub struct ExtractReport {
    pub index: String,
    pub batch_file: Option<String>,
    pub learnings_appended: usize,
    /// The head the boundary receipt recorded; `None` outside a repository.
    pub receipt_head: Option<String>,
}

const SESSION_DIR: &str = ".craftsman/session";

/// Write/refresh the session extract.
///
/// # Errors
/// [`SessionError::Io`] on filesystem failure; git being absent is not an
/// error (the index says so instead).
pub fn extract(
    root: &Path,
    config: &Config,
    req: &ExtractRequest,
) -> Result<ExtractReport, SessionError> {
    let dir = root.join(SESSION_DIR);
    std::fs::create_dir_all(&dir).map_err(|source| SessionError::Io {
        path: dir.clone(),
        source,
    })?;
    let now = iso_utc_now();
    let active = active_files(root);
    let plan = plan_progress(root, config);

    let batch_file = match req.batch {
        Some(n) => {
            let path = dir.join(format!("batch-{n}.md"));
            append_batch_file(&path, n, &now, req, active.as_deref())?;
            Some(format!("{SESSION_DIR}/batch-{n}.md"))
        }
        None => None,
    };

    let learnings_appended = append_learnings(&dir, req, &now)?;

    let index = render_index(&now, req, plan.as_ref(), active.as_deref(), &dir);
    let index_path = dir.join("index.md");
    std::fs::write(&index_path, index).map_err(|source| SessionError::Io {
        path: index_path,
        source,
    })?;
    let receipt_head = write_receipt(&dir, root, &now)?;

    Ok(ExtractReport {
        index: format!("{SESSION_DIR}/index.md"),
        batch_file,
        learnings_appended,
        receipt_head,
    })
}

/// Append failed approaches to the append-only `learnings.md`.
fn append_learnings(dir: &Path, req: &ExtractRequest, now: &str) -> Result<usize, SessionError> {
    if req.failed.is_empty() {
        return Ok(0);
    }
    let path = dir.join("learnings.md");
    let mut text = if path.is_file() {
        std::fs::read_to_string(&path).map_err(|source| SessionError::Io {
            path: path.clone(),
            source,
        })?
    } else {
        "# Learnings (append-only): failed approaches, surprises, gotchas\n\n".to_owned()
    };
    for item in &req.failed {
        let batch = req
            .batch
            .map_or_else(String::new, |n| format!("batch {n}, "));
        let _ = writeln!(text, "- [{batch}{now}] {item}");
    }
    std::fs::write(&path, text).map_err(|source| SessionError::Io { path, source })?;
    Ok(req.failed.len())
}

/// The content of `index.md`, for `extract --show`.
///
/// # Errors
/// [`SessionError::NoIndex`] when no extract has been written yet.
pub fn show(root: &Path) -> Result<String, SessionError> {
    let path = root.join(SESSION_DIR).join("index.md");
    if !path.is_file() {
        return Err(SessionError::NoIndex);
    }
    std::fs::read_to_string(&path).map_err(|source| SessionError::Io { path, source })
}

/// Plan checkbox progress — the only "inference", and it is arithmetic.
#[derive(Debug)]
struct PlanProgress {
    file: String,
    done: usize,
    open: usize,
}

fn plan_progress(root: &Path, config: &Config) -> Option<PlanProgress> {
    let file = config.project.plan.clone();
    let text = std::fs::read_to_string(root.join(&file)).ok()?;
    let done = text.matches("- [x]").count();
    let open = text.matches("- [ ]").count();
    Some(PlanProgress { file, done, open })
}

/// Modified/untracked paths from `git status --porcelain`; `None` when git
/// is unavailable or this is not a repository.
fn active_files(root: &Path) -> Option<Vec<String>> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|l| l.get(3..).map(str::to_owned))
            .collect(),
    )
}

fn render_index(
    now: &str,
    req: &ExtractRequest,
    plan: Option<&PlanProgress>,
    active: Option<&[String]>,
    dir: &Path,
) -> String {
    let mut out = format!("# Session State\n\nUpdated: {now}\n\n## Current Position\n");
    match req.batch {
        Some(n) => {
            let _ = writeln!(out, "- Batch {n} boundary extract");
        }
        None => out.push_str("- (no batch recorded this extract)\n"),
    }
    match plan {
        Some(p) => {
            let _ = writeln!(
                out,
                "- Plan {}: {} checkboxes done, {} open",
                p.file, p.done, p.open
            );
        }
        None => out.push_str("- Plan: not readable\n"),
    }

    out.push_str("\n## Key Decisions This Session\n");
    if req.decisions.is_empty() {
        out.push_str("- (none recorded)\n");
    }
    for d in &req.decisions {
        let _ = writeln!(out, "- {d}");
    }

    out.push_str("\n## Active Files (git status)\n");
    match active {
        None => out.push_str("- (not a git repository)\n"),
        Some([]) => out.push_str("- (clean tree)\n"),
        Some(files) => {
            for f in files {
                let _ = writeln!(out, "- {f}");
            }
        }
    }

    out.push_str("\n## Open Questions\n");
    if req.open.is_empty() {
        out.push_str("- (none recorded)\n");
    }
    for q in &req.open {
        let _ = writeln!(out, "- {q}");
    }

    out.push_str("\n## Read On Demand\n");
    let mut extras: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| n != "index.md" && crate::docs::is_md(n))
        .collect();
    // The files this very extract writes may not exist yet at render time.
    if let Some(n) = req.batch {
        extras.push(format!("batch-{n}.md"));
    }
    if !req.failed.is_empty() {
        extras.push("learnings.md".to_owned());
    }
    extras.sort();
    extras.dedup();
    if extras.is_empty() {
        out.push_str("- (nothing extracted yet)\n");
    }
    for e in extras {
        let _ = writeln!(out, "- {SESSION_DIR}/{e}");
    }
    out
}

fn append_batch_file(
    path: &Path,
    batch: u32,
    now: &str,
    req: &ExtractRequest,
    active: Option<&[String]>,
) -> Result<(), SessionError> {
    let mut text = if path.is_file() {
        std::fs::read_to_string(path).map_err(|source| SessionError::Io {
            path: path.to_path_buf(),
            source,
        })?
    } else {
        format!("# Batch {batch} — session extracts\n")
    };
    let _ = write!(text, "\n## Extract {now}\n");
    for (title, items) in [
        ("Decisions", &req.decisions),
        ("Failed approaches", &req.failed),
        ("Open questions", &req.open),
    ] {
        if items.is_empty() {
            continue;
        }
        let _ = write!(text, "\n### {title}\n");
        for item in items {
            let _ = writeln!(text, "- {item}");
        }
    }
    if let Some(files) = active
        && !files.is_empty()
    {
        let _ = write!(text, "\n### Files in flight\n");
        for f in files {
            let _ = writeln!(text, "- {f}");
        }
    }
    std::fs::write(path, text).map_err(|source| SessionError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::from_toml(
            "[project]\nname = \"demo\"\nstacks = [\"rust\"]\n",
            Path::new("craftsman.toml"),
        )
        .expect("minimal config")
    }

    #[test]
    fn extract_writes_index_batch_and_learnings() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("PLAN.md"),
            "# Plan\n- [x] one\n- [x] two\n- [ ] three\n",
        )
        .expect("plan");
        let req = ExtractRequest {
            batch: Some(7),
            decisions: vec!["chose curl over ureq".to_owned()],
            failed: vec!["probing hono per-page .md — 404".to_owned()],
            open: vec!["context7 pin syntax".to_owned()],
        };
        let report = extract(tmp.path(), &config(), &req).expect("extract");
        assert_eq!(report.learnings_appended, 1);
        let index = show(tmp.path()).expect("index exists");
        assert!(index.contains("Batch 7 boundary extract"), "{index}");
        assert!(index.contains("2 checkboxes done, 1 open"), "{index}");
        assert!(index.contains("chose curl over ureq"), "{index}");
        assert!(index.contains("context7 pin syntax"), "{index}");
        assert!(index.contains("(not a git repository)"), "{index}");
        assert!(index.contains("batch-7.md"), "{index}");
        let batch = std::fs::read_to_string(tmp.path().join(".craftsman/session/batch-7.md"))
            .expect("batch file");
        assert!(batch.contains("### Failed approaches"), "{batch}");
    }

    #[test]
    fn learnings_accumulate_across_extracts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let one = ExtractRequest {
            batch: Some(1),
            failed: vec!["first dead end".to_owned()],
            ..Default::default()
        };
        let two = ExtractRequest {
            batch: Some(2),
            failed: vec!["second dead end".to_owned()],
            ..Default::default()
        };
        extract(tmp.path(), &config(), &one).expect("first");
        extract(tmp.path(), &config(), &two).expect("second");
        let text = std::fs::read_to_string(tmp.path().join(".craftsman/session/learnings.md"))
            .expect("learnings");
        assert!(text.contains("first dead end"), "append-only: {text}");
        assert!(text.contains("second dead end"), "{text}");
        let index = show(tmp.path()).expect("index");
        assert!(
            index.contains("batch-1.md") && index.contains("batch-2.md"),
            "{index}"
        );
    }

    #[test]
    fn show_without_extract_is_a_loud_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = show(tmp.path()).expect_err("no index yet");
        assert!(matches!(err, SessionError::NoIndex), "{err}");
    }
}

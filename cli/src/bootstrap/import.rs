//! `craftsman import` — the ADR-006 entry gear for foreign trees.
//!
//! Brings a tree that arrived from elsewhere (copied sibling, forked or
//! vendored open source) under the system: scaffold the contract
//! non-destructively, audit the flaws in full, and detect existing QA
//! commands as `[gates.qa]` conversion candidates. Debt disposal stays
//! explicit and human-gated — import never records a baseline.

use std::path::Path;

use serde::Serialize;

use super::init::{self, FileReport, InitError, Request};

/// Everything import wrote and found.
#[derive(Debug, Serialize)]
pub struct Report {
    pub root: String,
    /// Scaffold outcomes; existing files are `kept`, never overwritten.
    pub files: Vec<FileReport>,
    /// package.json script names — candidates for `[gates.qa]` conversion.
    pub qa_candidates: Vec<String>,
    pub next: Vec<String>,
}

/// Scaffold the contract into an existing tree.
///
/// Non-destructive by construction: every scaffold target that already
/// exists is kept as-is and reported, the `.gitignore` is merged, and the
/// `CLAUDE.md` symlink is only created when absent.
///
/// # Errors
/// [`InitError`] for unknown stacks, a missing git repository, or IO.
pub fn run(cwd: &Path, request: &Request) -> Result<Report, InitError> {
    for stack in &request.stacks {
        if !init::KNOWN_STACKS.contains(&stack.as_str()) {
            return Err(InitError::UnknownStack {
                stack: stack.clone(),
            });
        }
    }
    if !cwd.join(".git").exists() {
        return Err(InitError::NotAGitRepo {
            dir: cwd.to_path_buf(),
        });
    }

    let mut files = scaffold_kept(cwd, request)?;
    let claude = cwd.join("CLAUDE.md");
    if claude.exists() || claude.is_symlink() {
        files.push(FileReport {
            path: "CLAUDE.md".to_owned(),
            action: "kept",
        });
    } else {
        files.push(init::claude_md_link(cwd, false)?);
    }
    files.push(init::merge_gitignore(cwd)?);
    for dir in ["baselines", "session", "cache", "docs"] {
        let path = cwd.join(".craftsman").join(dir);
        std::fs::create_dir_all(&path).map_err(|source| InitError::Io { path, source })?;
    }

    Ok(Report {
        root: cwd.display().to_string(),
        files,
        qa_candidates: qa_candidates(cwd),
        next: vec![
            "audit the inherited debt: craftsman import --audit (report only, \
             nothing is baselined)"
                .to_owned(),
            "dispose of the debt explicitly: remediation batches in PLAN.md by \
             default; `craftsman gate baseline <gate>` only with a recorded reason"
                .to_owned(),
            "convert detected QA commands into [gates.qa] entries so check-all \
             carries the project's real acceptance"
                .to_owned(),
        ],
    })
}

/// Existing QA command candidates: package.json script names, reported for
/// human conversion into `[gates.qa]` — detection only, never acted on.
fn qa_candidates(cwd: &Path) -> Vec<String> {
    std::fs::read_to_string(cwd.join("package.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|doc| {
            doc.get("scripts")
                .and_then(|s| s.as_object())
                .map(|m| m.keys().cloned().collect())
        })
        .unwrap_or_default()
}

/// Write each scaffold target only where nothing exists — the
/// non-destructive core: an existing file is `kept`, never overwritten.
fn scaffold_kept(cwd: &Path, request: &Request) -> Result<Vec<FileReport>, InitError> {
    let mut files = Vec::new();
    for (rel, content) in super::templates::targets(request, env!("CARGO_PKG_VERSION")) {
        let path = cwd.join(&rel);
        if path.exists() || path.is_symlink() {
            files.push(FileReport {
                path: rel,
                action: "kept",
            });
        } else {
            init::write_file(&path, &content)?;
            files.push(FileReport {
                path: rel,
                action: "created",
            });
        }
    }
    Ok(files)
}

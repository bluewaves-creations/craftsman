//! `craftsman init` — non-interactive greenfield scaffold.
//!
//! Flags in, files out (skill-family design: the CLI scaffolds, the
//! craftsman-init skill drives the interview). Writes craftsman.toml, the
//! AGENTS.md skeleton, a walking-skeleton SPEC.md, `.craftsman/` dirs,
//! `.gitignore` entries, the CLAUDE.md symlink, and harness hook
//! templates. Refuses over existing files without `--force` — the
//! destructive confirmation is skill-side; the CLI just refuses loudly.

use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use super::templates;

/// Stacks `[project] stacks` accepts (design doc).
pub const KNOWN_STACKS: &[&str] = &[
    "swift-apple",
    "swift",
    "python",
    "typescript",
    "rust",
    "bash",
];

/// What `craftsman init` was asked to scaffold.
#[derive(Debug)]
pub struct Request {
    pub name: String,
    pub stacks: Vec<String>,
    /// Spec file name; `None` picks the stack-appropriate default
    /// ([`default_spec`]).
    pub spec: Option<String>,
    pub force: bool,
}

/// The default spec path per stack set.
///
/// The typescript runner (cucumber-js) discovers `features/**/*.feature`
/// and never reads a markdown spec — scaffolding `SPEC.md` there yields
/// 0 scenarios and exit 4 on the first verify (craftsman-web ledger
/// finding 1). Every other stack keeps `SPEC.md`.
#[must_use]
pub fn default_spec(name: &str, stacks: &[String]) -> String {
    if stacks.iter().any(|s| s == "typescript") {
        format!("features/{name}.feature")
    } else {
        "SPEC.md".to_owned()
    }
}

/// Errors are exit-3 territory: nothing was scaffolded.
#[derive(Debug, Error)]
pub enum InitError {
    #[error(
        "{dir} is not a git repository — the ledger needs git; run \
         `git init` first, then `craftsman init` again"
    )]
    NotAGitRepo { dir: PathBuf },
    #[error("unknown stack {stack:?} — known stacks: {}", KNOWN_STACKS.join(", "))]
    UnknownStack { stack: String },
    #[error(
        "refusing to overwrite existing file(s):\n{}\n\
         re-run with --force to overwrite them (confirm the destructive \
         scope first — that judgment is the skill's, not the CLI's)",
        files.iter().map(|f| format!("  {f}")).collect::<Vec<_>>().join("\n")
    )]
    WouldOverwrite { files: Vec<String> },
    #[error("cannot write {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// One scaffolded file in the report.
#[derive(Debug, Serialize)]
pub struct FileReport {
    pub path: String,
    /// `created | merged | symlinked | pointer-file | overwritten`.
    pub action: &'static str,
}

/// Everything init wrote.
#[derive(Debug, Serialize)]
pub struct Report {
    pub root: String,
    pub files: Vec<FileReport>,
    pub next: Vec<String>,
}

/// The scaffold target list: (relative path, content). `.gitignore` is
/// handled separately (merged, never a conflict).
fn targets(request: &Request, version: &str) -> Vec<(String, String)> {
    let stacks = request
        .stacks
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let spec_rel = request
        .spec
        .clone()
        .unwrap_or_else(|| default_spec(&request.name, &request.stacks));
    let config = templates::INIT_CONFIG_TOML
        .replace("__NAME__", &request.name)
        .replace("__STACKS__", &stacks)
        .replace("__SPEC__", &spec_rel)
        .replace("__VERSION__", version);
    let agents = templates::AGENTS_MD.replace("__NAME__", &request.name);
    let spec = templates::SPEC_MD.replace("__NAME__", &request.name);
    vec![
        ("craftsman.toml".to_owned(), config),
        ("AGENTS.md".to_owned(), agents),
        (spec_rel, spec),
        (
            ".claude/settings.json".to_owned(),
            templates::CLAUDE_SETTINGS_JSON.to_owned(),
        ),
        (
            ".cursor/hooks.json".to_owned(),
            templates::CURSOR_HOOKS_JSON.to_owned(),
        ),
    ]
}

/// Run the scaffold in `cwd`.
///
/// # Errors
/// [`InitError::NotAGitRepo`] (exit 3, suggests `git init`);
/// [`InitError::WouldOverwrite`] listing conflicts without `--force`;
/// [`InitError::UnknownStack`]; IO failures.
pub fn run(cwd: &Path, request: &Request) -> Result<Report, InitError> {
    for stack in &request.stacks {
        if !KNOWN_STACKS.contains(&stack.as_str()) {
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

    let version = env!("CARGO_PKG_VERSION");
    let files = targets(request, version);

    // Refusal pass first: nothing is written while any conflict stands.
    let conflicts: Vec<String> = files
        .iter()
        .map(|(rel, _)| rel.clone())
        .chain(std::iter::once("CLAUDE.md".to_owned()))
        .filter(|rel| cwd.join(rel).exists() || cwd.join(rel).is_symlink())
        .collect();
    if !conflicts.is_empty() && !request.force {
        return Err(InitError::WouldOverwrite { files: conflicts });
    }

    let mut report = Vec::new();
    for (rel, content) in &files {
        let path = cwd.join(rel);
        let existed = path.exists();
        write_file(&path, content)?;
        report.push(FileReport {
            path: rel.clone(),
            action: if existed { "overwritten" } else { "created" },
        });
    }
    report.push(claude_md_link(cwd, request.force)?);
    report.push(merge_gitignore(cwd)?);
    for dir in ["baselines", "session", "cache", "docs"] {
        let path = cwd.join(".craftsman").join(dir);
        std::fs::create_dir_all(&path).map_err(|source| InitError::Io { path, source })?;
    }

    Ok(Report {
        root: cwd.display().to_string(),
        files: report,
        next: vec![
            "fill AGENTS.md through the craftsman-init interview (human-attested content only)"
                .to_owned(),
            "wire the stack's verify runner, then `craftsman doctor` to prove the loop closes"
                .to_owned(),
            "first ledger commit: git add -A && craftsman commit --type chore --message \
             \"bring repo under craftsman\""
                .to_owned(),
        ],
    })
}

fn write_file(path: &Path, content: &str) -> Result<(), InitError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| InitError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, content).map_err(|source| InitError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// CLAUDE.md → AGENTS.md: a relative symlink (the one tolerated harness
/// artifact), falling back to a pointer file where symlinks fail.
fn claude_md_link(cwd: &Path, force: bool) -> Result<FileReport, InitError> {
    let link = cwd.join("CLAUDE.md");
    if force && (link.exists() || link.is_symlink()) {
        std::fs::remove_file(&link).map_err(|source| InitError::Io {
            path: link.clone(),
            source,
        })?;
    }
    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink("AGENTS.md", &link).is_ok() {
            return Ok(FileReport {
                path: "CLAUDE.md".to_owned(),
                action: "symlinked",
            });
        }
    }
    write_file(&link, templates::CLAUDE_POINTER_MD)?;
    Ok(FileReport {
        path: "CLAUDE.md".to_owned(),
        action: "pointer-file",
    })
}

/// Append the missing `.craftsman/` ignore lines; existing content is
/// never rewritten (merging is not a conflict).
fn merge_gitignore(cwd: &Path) -> Result<FileReport, InitError> {
    let path = cwd.join(".gitignore");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let missing: Vec<&str> = templates::GITIGNORE_LINES
        .iter()
        .copied()
        .filter(|line| !existing.lines().any(|l| l.trim() == *line))
        .collect();
    if missing.is_empty() {
        return Ok(FileReport {
            path: ".gitignore".to_owned(),
            action: "merged",
        });
    }
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&missing.join("\n"));
    text.push('\n');
    write_file(&path, &text)?;
    Ok(FileReport {
        path: ".gitignore".to_owned(),
        action: "merged",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> Request {
        Request {
            name: "demo".to_owned(),
            stacks: vec!["rust".to_owned()],
            spec: None,
            force: false,
        }
    }

    fn git_dir(dir: &Path) {
        std::fs::create_dir_all(dir.join(".git")).expect("fake .git");
    }

    #[test]
    fn refuses_outside_a_git_repo() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = run(tmp.path(), &request()).expect_err("no .git");
        assert!(matches!(err, InitError::NotAGitRepo { .. }), "{err}");
        assert!(format!("{err}").contains("git init"));
    }

    #[test]
    fn rejects_unknown_stacks() {
        let tmp = tempfile::tempdir().expect("tempdir");
        git_dir(tmp.path());
        let mut req = request();
        req.stacks = vec!["cobol".to_owned()];
        let err = run(tmp.path(), &req).expect_err("unknown stack");
        assert!(matches!(err, InitError::UnknownStack { .. }), "{err}");
    }

    #[test]
    fn scaffolds_a_config_doctor_accepts_and_lists_conflicts_second_time() {
        let tmp = tempfile::tempdir().expect("tempdir");
        git_dir(tmp.path());
        let report = run(tmp.path(), &request()).expect("scaffold");
        assert!(report.files.iter().any(|f| f.path == "craftsman.toml"));

        // The written config parses and keeps verify strict.
        let loaded = crate::config::Config::load(tmp.path()).expect("config loads");
        assert_eq!(
            loaded.config.gates.verify,
            Some(crate::config::GateMode::Strict)
        );
        // The walking skeleton parses and lints clean.
        let feature = crate::spec::parse_spec(&tmp.path().join("SPEC.md")).expect("spec parses");
        assert!(
            crate::spec::lint(&feature)
                .iter()
                .all(|f| f.severity != crate::spec::Severity::Error),
            "walking skeleton must lint clean"
        );
        // The hooks template is valid JSON in the verified settings shape.
        let text = std::fs::read_to_string(tmp.path().join(".claude/settings.json"))
            .expect("settings written");
        let doc: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert!(
            doc["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
                .as_str()
                .expect("command hook")
                .contains("check-all --changed")
        );
        assert!(doc["hooks"]["Stop"].is_array());

        // Second run refuses and lists the conflicts.
        let err = run(tmp.path(), &request()).expect_err("must refuse");
        let InitError::WouldOverwrite { files } = &err else {
            panic!("expected WouldOverwrite, got {err}");
        };
        assert!(files.iter().any(|f| f == "craftsman.toml"), "{files:?}");
        assert!(files.iter().any(|f| f == "AGENTS.md"), "{files:?}");
    }

    #[test]
    fn force_overwrites_and_gitignore_merges_idempotently() {
        let tmp = tempfile::tempdir().expect("tempdir");
        git_dir(tmp.path());
        std::fs::write(tmp.path().join(".gitignore"), "target/\n").expect("seed");
        run(tmp.path(), &request()).expect("first");
        let mut req = request();
        req.force = true;
        run(tmp.path(), &req).expect("forced second run");
        let ignore = std::fs::read_to_string(tmp.path().join(".gitignore")).expect("read");
        assert_eq!(
            ignore.matches(".craftsman/cache/").count(),
            1,
            "merge must be idempotent:\n{ignore}"
        );
        assert!(ignore.starts_with("target/\n"), "existing content kept");
    }

    #[cfg(unix)]
    #[test]
    fn claude_md_is_a_symlink_to_agents_md() {
        let tmp = tempfile::tempdir().expect("tempdir");
        git_dir(tmp.path());
        run(tmp.path(), &request()).expect("scaffold");
        let link = tmp.path().join("CLAUDE.md");
        let target = std::fs::read_link(&link).expect("CLAUDE.md is a symlink");
        assert_eq!(target, PathBuf::from("AGENTS.md"));
    }
}

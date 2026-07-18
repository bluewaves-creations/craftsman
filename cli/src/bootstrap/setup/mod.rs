//! `craftsman setup` — install the bundled six skills (design decision
//! #3: skills embedded in the binary via `include_dir`, the hatch-wheel
//! pattern translated to cargo).
//!
//! Canonical home: `~/.agents/skills/` (the open agent-skills standard
//! location). A per-agent adapter table — ported from Fusion's setup.py —
//! then fans out: `link` agents get a symlink per skill in their own
//! skills dir; `standard` agents read the canonical dir natively, so
//! creating links there too would load every skill twice.
//!
//! ATTRIBUTION-CHECKED never-destroy: setup only replaces symlinks that
//! resolve into the canonical dir, and directory trees it can prove it
//! wrote — a `.craftsman-setup` sentinel recording the sha256 of the tree,
//! or a tree digest equal to the payload's. Foreign content is reported
//! and left; `--force` overrides, still listing what it replaced.

mod attest;
mod ops;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use include_dir::{Dir, include_dir};
use serde::Serialize;
use thiserror::Error;

/// The whole `skills/` tree, embedded at build time.
static SKILLS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../skills");

/// Provenance sentinel written into every copy setup creates.
pub const SENTINEL: &str = ".craftsman-setup";

/// How an agent consumes skills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Does not read `~/.agents/skills` — each skill gets a symlink in
    /// the agent's own skills dir.
    Link,
    /// Reads the canonical dir natively — nothing to install.
    Standard,
}

/// One row of the agent adapter table.
#[derive(Debug)]
pub struct AgentSpec {
    pub name: &'static str,
    /// Home-relative detection marker directory.
    pub marker: &'static str,
    /// Home-relative skills dir the agent reads.
    pub skills_subdir: &'static str,
    pub mode: Mode,
}

/// The adapter table (ported from Fusion's setup.py, 2026-07).
pub const AGENTS: &[AgentSpec] = &[
    AgentSpec {
        name: "Claude Code",
        marker: ".claude",
        skills_subdir: ".claude/skills",
        mode: Mode::Link,
    },
    AgentSpec {
        name: "Codex",
        marker: ".codex",
        skills_subdir: ".agents/skills",
        mode: Mode::Standard,
    },
    AgentSpec {
        name: "Pi",
        marker: ".pi",
        skills_subdir: ".agents/skills",
        mode: Mode::Standard,
    },
    AgentSpec {
        name: "Cursor",
        marker: ".cursor",
        skills_subdir: ".agents/skills",
        mode: Mode::Standard,
    },
    AgentSpec {
        name: "Gemini CLI",
        marker: ".gemini",
        skills_subdir: ".agents/skills",
        mode: Mode::Standard,
    },
    AgentSpec {
        name: "opencode",
        marker: ".config/opencode",
        skills_subdir: ".agents/skills",
        mode: Mode::Standard,
    },
    AgentSpec {
        name: "Goose",
        marker: ".config/goose",
        skills_subdir: ".agents/skills",
        mode: Mode::Standard,
    },
];

#[derive(Debug, Error)]
pub enum SetupError {
    #[error("HOME is not set — setup installs under the user home")]
    NoHome,
    #[error(
        "no sha256 tool (shasum/sha256sum) — attribution needs content \
         hashes; refusing to guess what setup may replace"
    )]
    DigestUnavailable,
    #[error("the embedded skills payload is missing {what} — rebuild craftsman")]
    BadPayload { what: String },
    #[error("cannot read or write {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// One action row in the setup report.
#[derive(Debug, Serialize)]
pub struct Row {
    /// `canonical` or the agent name.
    pub scope: String,
    pub skill: String,
    pub action: &'static str,
    pub detail: String,
}

/// The setup report.
#[derive(Debug, Serialize)]
pub struct Report {
    pub version: &'static str,
    pub canonical_dir: String,
    pub rows: Vec<Row>,
}

pub use ops::{install, remove, status};

/// The user home (honors `$HOME`, so tests and sandboxes redirect it).
///
/// # Errors
/// [`SetupError::NoHome`] when `$HOME` is unset.
pub fn home() -> Result<PathBuf, SetupError> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(SetupError::NoHome)
}

/// `~/.agents/skills` — the canonical install target.
#[must_use]
pub fn canonical_dir(home: &Path) -> PathBuf {
    home.join(".agents/skills")
}

/// The embedded `craftsman-*` skill directories, sorted by name.
///
/// # Errors
/// [`SetupError::BadPayload`] when the embed does not hold exactly six.
pub fn payload_skills() -> Result<Vec<&'static Dir<'static>>, SetupError> {
    let mut dirs: Vec<&Dir<'_>> = SKILLS
        .dirs()
        .filter(|d| skill_name(d).starts_with("craftsman-"))
        .collect();
    dirs.sort_by_key(|d| skill_name(d));
    if dirs.len() != 6 {
        return Err(SetupError::BadPayload {
            what: format!("expected 6 craftsman-* skills, found {}", dirs.len()),
        });
    }
    Ok(dirs)
}

/// A payload dir's basename (`include_dir` paths are embed-root-relative).
#[must_use]
pub fn skill_name<'a>(dir: &'a Dir<'a>) -> &'a str {
    dir.path()
        .file_name()
        .map_or("", |n| n.to_str().unwrap_or(""))
}

/// An embedded payload file by embed-root-relative path (byte-identity
/// checks and drift reports read the payload through this).
#[must_use]
pub fn embedded_file(path: &str) -> Option<&'static [u8]> {
    SKILLS.get_file(path).map(include_dir::File::contents)
}

/// Collect an embedded dir's files as (skill-relative path, bytes),
/// sorted — the digest input and the extraction worklist.
#[must_use]
pub fn payload_files(dir: &Dir<'static>) -> Vec<(String, &'static [u8])> {
    fn walk(dir: &Dir<'static>, base: &Path, out: &mut Vec<(String, &'static [u8])>) {
        for file in dir.files() {
            let rel = file
                .path()
                .strip_prefix(base)
                .unwrap_or_else(|_| file.path());
            out.push((rel.to_string_lossy().into_owned(), file.contents()));
        }
        for sub in dir.dirs() {
            walk(sub, base, out);
        }
    }
    let mut out = Vec::new();
    walk(dir, dir.path(), &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// sha256 of a (relpath, bytes) stream via the system tool — same recipe
/// for embedded payloads and on-disk trees, so digests are comparable.
///
/// # Errors
/// [`SetupError::DigestUnavailable`] without a system sha256 tool.
pub fn digest_entries(entries: &[(String, Vec<u8>)]) -> Result<String, SetupError> {
    let mut material: Vec<u8> = Vec::new();
    for (rel, bytes) in entries {
        material.extend_from_slice(rel.as_bytes());
        material.push(0);
        material.extend_from_slice(bytes);
        material.push(0);
    }
    sha256_bytes(&material)
}

/// Digest of an embedded skill dir.
///
/// # Errors
/// [`SetupError::DigestUnavailable`] without a system sha256 tool.
pub fn payload_digest(dir: &Dir<'static>) -> Result<String, SetupError> {
    let entries: Vec<(String, Vec<u8>)> = payload_files(dir)
        .into_iter()
        .map(|(rel, bytes)| (rel, bytes.to_vec()))
        .collect();
    digest_entries(&entries)
}

/// Digest of an on-disk tree, excluding the sentinel itself.
///
/// # Errors
/// IO failures reading the tree; [`SetupError::DigestUnavailable`].
pub fn tree_digest(root: &Path) -> Result<String, SetupError> {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    collect_tree(root, root, &mut entries)?;
    entries.retain(|(rel, _)| rel != SENTINEL);
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    digest_entries(&entries)
}

fn collect_tree(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), SetupError> {
    let read = |p: &Path| {
        std::fs::read(p).map_err(|source| SetupError::Io {
            path: p.to_path_buf(),
            source,
        })
    };
    let iter = std::fs::read_dir(dir).map_err(|source| SetupError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in iter {
        let entry = entry.map_err(|source| SetupError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_tree(root, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            out.push((rel, read(&path)?));
        }
    }
    Ok(())
}

/// sha256 over bytes via `shasum -a 256` / `sha256sum` stdin (the repo's
/// no-new-crates hashing convention; FNV is for cache keys only).
fn sha256_bytes(bytes: &[u8]) -> Result<String, SetupError> {
    use std::io::Write as _;
    for (program, args) in [("shasum", &["-a", "256"][..]), ("sha256sum", &[][..])] {
        let child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else { continue };
        if let Some(stdin) = child.stdin.as_mut()
            && stdin.write_all(bytes).is_err()
        {
            continue;
        }
        if let Ok(output) = child.wait_with_output()
            && output.status.success()
            && let Some(hash) = String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .next()
        {
            return Ok(hash.to_owned());
        }
    }
    Err(SetupError::DigestUnavailable)
}

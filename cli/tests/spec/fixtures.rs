//! The shared fixture vocabulary of this harness. A stable fixture
//! follows one lifecycle: `stable_dir(name)` → `scrub` previous-run
//! state → build → (when a repository is needed) `git_init_commit_all`
//! or `recommit_scaffold`. The traps these helpers encode are recorded
//! in README.md next door.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A per-scenario fixture directory at a stable temp path, so compiled
/// state (`target/`, `.venv`) survives across runs. Every scenario gets
/// its own name — concurrently running scenarios never share a fixture.
pub fn stable_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

/// Remove previous-run state (files or directories; missing entries are
/// fine). Anything a scenario writes into a stable fixture must be
/// scrubbed on entry, or the second full run inherits the first.
pub fn scrub(dir: &Path, entries: &[&str]) {
    for entry in entries {
        let path = dir.join(entry);
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Run one git command in `dir`, asserting success.
pub fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("spawn git");
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

/// Run one git command in `dir` and return its stdout.
pub fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed in {}",
        dir.display()
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `git init` + `git add -A` (arch and health census tracked files via
/// `git ls-files`; no commit needed).
pub fn git_init_add(dir: &Path) {
    git(dir, &["init", "--quiet"]);
    git(dir, &["add", "-A"]);
}

/// Write the fixture identity into the repo config — never per-command
/// `-c` flags: commits the CLI itself makes later must resolve the same
/// identity, and CI runners have no global one.
pub fn git_identity(dir: &Path) {
    git(dir, &["config", "user.name", "fixture"]);
    git(dir, &["config", "user.email", "fixture@example.invalid"]);
}

/// Fresh single-commit repository: init, stage everything, identity,
/// commit — for fixtures that need a resolvable `HEAD`.
pub fn git_init_commit_all(dir: &Path) {
    git_init_add(dir);
    git_identity(dir);
    git(dir, &["commit", "--quiet", "-m", "init"]);
}

/// Rebuild a scaffolded stable fixture as a fresh single-commit
/// repository and return its `HEAD`. Scrubs the state previous runs
/// leave behind first — a `NOTES.md` riding into the initial commit
/// means staging the identical file later stages nothing — and ignores
/// build/state dirs so the tree stays clean.
pub fn recommit_scaffold(dir: &Path) -> String {
    scrub(dir, &["NOTES.md", ".git"]);
    std::fs::write(dir.join(".gitignore"), "target/\n.craftsman/\nCargo.lock\n")
        .expect("write .gitignore");
    git_init_commit_all(dir);
    git_stdout(dir, &["rev-parse", "HEAD"]).trim().to_owned()
}

/// Recursive fixture copy, skipping caches and per-run state (`.git`,
/// `.craftsman`, `.venv`, `node_modules`, `__pycache__`, `target`).
pub fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdirs");
    for entry in std::fs::read_dir(from).expect("read fixture dir") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name();
        let skip = [
            ".git",
            ".craftsman",
            ".venv",
            "node_modules",
            "__pycache__",
            "target",
        ];
        if skip.iter().any(|s| name.to_string_lossy() == *s) {
            continue;
        }
        let src = entry.path();
        let dest = to.join(&name);
        if src.is_dir() {
            copy_tree(&src, &dest);
        } else {
            std::fs::copy(&src, &dest).unwrap_or_else(|e| panic!("copy {}: {e}", src.display()));
        }
    }
}

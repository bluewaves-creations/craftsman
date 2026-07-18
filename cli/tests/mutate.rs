//! Mutate-gate e2e through the real CLI path (`craftsman mutate --json`)
//! for the python (mutmut 2.5.1) and typescript (Stryker incremental)
//! stacks — closing the Batch 6b honest-undone: parsers were unit-tested,
//! these two command paths now run live.
//!
//! Each test assembles a disposable git project from committed fixture
//! pieces (`fixtures/python-todo/mutation/`, `fixtures/ts-todo/src/` +
//! `stryker.config.json`), seeds a diff by appending a comment to the
//! mutation target, and asserts score parsing + the threshold verdict at
//! `[mutate] min-score = 100` — the weak tests guarantee boundary-mutant
//! survivors, so the red verdict is deterministic while exact scores are
//! tool-version detail.
//!
//! Timing decision (measured on this machine, 2026-07-18): python 2.1s
//! warm, typescript 1.1s warm; the first-ever run pays one-time bunx
//! Stryker + uv env resolution (~60s observed cold). Both far under the
//! 90s ignore threshold — they run unignored. mutmut's aggregate-only
//! survivor report (no per-line detail on python >= 3.13) is the
//! documented ADR-004 limitation and is asserted as such.
//!
//! Requirements: `uv` and `bun`/`bunx` on PATH (AGENTS.md toolchain);
//! network only on the very first bunx/uv resolution of the pinned tools.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdirs");
    }
    std::fs::write(&path, content).unwrap_or_else(|e| panic!("write {rel}: {e}"));
}

fn copy_from_fixture(fixture_dir: &Path, rel: &str, dir: &Path, dest_rel: &str) {
    let content = std::fs::read_to_string(fixture_dir.join(rel))
        .unwrap_or_else(|e| panic!("read fixture {rel}: {e}"));
    write(dir, dest_rel, &content);
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("spawn git");
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

fn commit_all(dir: &Path) {
    git(dir, &["init", "--quiet"]);
    git(dir, &["add", "-A"]);
    git(
        dir,
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "base",
        ],
    );
}

fn seed_diff(dir: &Path, rel: &str, comment: &str) {
    let path = dir.join(rel);
    let mut text = std::fs::read_to_string(&path).expect("read target");
    text.push_str(comment);
    std::fs::write(&path, text).expect("seed diff");
}

/// Run `craftsman mutate --json` in `dir`, returning (output, seconds).
fn run_mutate(dir: &Path) -> (Output, f64) {
    let started = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_craftsman"))
        .args(["mutate", "--json"])
        .current_dir(dir)
        .output()
        .expect("spawn craftsman");
    (output, started.elapsed().as_secs_f64())
}

/// Shared assertions: exit 1 (red at min-score 100), parseable JSON with a
/// parsed score note and survived-mutant findings from `tool`.
fn assert_red_with_survivors(output: &Output, elapsed: f64, stack: &str, tool: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected the survivor verdict (exit 1) after {elapsed:.0}s:\n{stdout}{stderr}"
    );
    let doc: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout not JSON ({e}):\n{stdout}"));
    assert_eq!(doc["gate"], "mutate");
    assert_eq!(doc["passed"], false);
    let notes = doc["notes"].as_array().expect("notes array");
    let score_note = notes
        .iter()
        .filter_map(|n| n.as_str())
        .find(|n| n.starts_with(&format!("mutate[{stack}]: score ")))
        .unwrap_or_else(|| panic!("no parsed score note for {stack} in {notes:?}"));
    assert!(
        score_note.contains("% —")
            && score_note.contains("missed")
            && score_note.contains("(threshold 100)"),
        "score note must carry the parsed tally and threshold: {score_note}"
    );
    let findings = doc["findings"].as_array().expect("findings array");
    assert!(
        findings
            .iter()
            .any(|f| f["tool"] == tool && f["rule"] == "survived-mutant"),
        "expected {tool} survived-mutant findings:\n{stdout}"
    );
    assert!(doc["blocking"].as_u64().unwrap_or(0) > 0, "{stdout}");
}

/// python-todo fixture pieces through the real CLI path: mutmut 2.5.1,
/// diff-scoped to the seeded change, aggregate survivor report (ADR-004).
#[test]
fn python_mutate_end_to_end_scores_and_blocks() {
    let fixture_dir = fixture("python-todo");
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    copy_from_fixture(&fixture_dir, "pyproject.toml", dir, "pyproject.toml");
    copy_from_fixture(&fixture_dir, "uv.lock", dir, "uv.lock");
    copy_from_fixture(&fixture_dir, "mutation/todo_util.py", dir, "todo_util.py");
    copy_from_fixture(
        &fixture_dir,
        "mutation/tests/test_util.py",
        dir,
        "tests/test_util.py",
    );
    write(
        dir,
        "craftsman.toml",
        "[project]\nname = \"mutate-py\"\nstacks = [\"python\"]\n\n[verify.python]\ntests-dir = \"tests\"\n\n[mutate]\nmin-score = 100\n",
    );
    commit_all(dir);
    seed_diff(dir, "todo_util.py", "\n# seeded diff for the mutate e2e\n");

    let (output, elapsed) = run_mutate(dir);
    assert_red_with_survivors(&output, elapsed, "python", "mutmut");
    // The documented mutmut 2.5.1 limitation: one aggregate finding, no
    // per-line detail (results browser broken on python >= 3.13).
    let stdout = String::from_utf8_lossy(&output.stdout);
    let doc: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    let survivor = doc["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|f| f["tool"] == "mutmut")
        .expect("mutmut finding");
    assert!(survivor["line"].is_null(), "aggregate-only: no line detail");
    assert!(
        survivor["message"]
            .as_str()
            .unwrap_or_default()
            .contains("mutant(s) survived in todo_util.py"),
        "{survivor}"
    );
}

/// ts-todo fixture pieces (committed stryker.config.json) through the real
/// CLI path: Stryker incremental, diff-scoped via --mutate, per-line
/// survivor findings from the mutation-testing-report-schema JSON.
#[test]
fn typescript_mutate_end_to_end_scores_and_blocks() {
    let fixture_dir = fixture("ts-todo");
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    copy_from_fixture(
        &fixture_dir,
        "stryker.config.json",
        dir,
        "stryker.config.json",
    );
    copy_from_fixture(&fixture_dir, "src/calc.ts", dir, "src/calc.ts");
    copy_from_fixture(&fixture_dir, "src/calc.test.ts", dir, "src/calc.test.ts");
    write(
        dir,
        "package.json",
        "{\n  \"name\": \"mutate-ts\",\n  \"private\": true\n}\n",
    );
    write(
        dir,
        "craftsman.toml",
        "[project]\nname = \"mutate-ts\"\nstacks = [\"typescript\"]\n\n[mutate]\nmin-score = 100\n",
    );
    write(dir, ".gitignore", ".stryker-tmp/\nreports/\n");
    commit_all(dir);
    seed_diff(dir, "src/calc.ts", "\n// seeded diff for the mutate e2e\n");

    let (output, elapsed) = run_mutate(dir);
    assert_red_with_survivors(&output, elapsed, "typescript", "stryker");
    // Stryker's report carries per-mutant locations — survivors have lines.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let doc: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    let survivor = doc["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|f| f["tool"] == "stryker")
        .expect("stryker finding");
    assert_eq!(survivor["file"], "src/calc.ts");
    assert!(survivor["line"].as_u64().is_some(), "{survivor}");
}

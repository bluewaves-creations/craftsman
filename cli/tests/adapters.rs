//! End-to-end adapter tests against the real runners, driving the committed
//! fixture projects under `tests/fixtures/{python-todo,ts-todo}/` through
//! `verify::run` (config load → stack dispatch → adapter → merge).
//!
//! Timing decision (measured on this machine, per the Batch 4 plan): a warm
//! python run is ~1.5s and a warm bun/cucumber-js run is ~1s, far under the
//! ~60s budget — so these run unconditionally, NOT `#[ignore]`d. The first
//! run on a fresh machine resolves the pinned environments from the
//! committed lockfiles (`uv.lock` / `bun.lock`), which may download into
//! the local uv/bun caches once. The artifact-only fast path (no runners)
//! lives in `tests/normalize_fixtures.rs` and the merge proof in
//! `pytest_bdd.rs` unit tests.
//!
//! Requirements: `uv` and `bun` on PATH (AGENTS.md toolchain).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use craftsman::verify::normalize::Status;
use craftsman::verify::{self, Outcome, Selection};

/// Verify runs against the same fixture project share its
/// `.craftsman/cache/verify` artifact paths — serialize them per fixture
/// (cargo runs test functions concurrently).
static PYTHON_TODO: Mutex<()> = Mutex::new(());
static TS_TODO: Mutex<()> = Mutex::new(());

fn fixture_project(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// The known trio every fixture project encodes: one passing scenario, one
/// failing assertion, one scenario with a deliberately unimplemented step.
fn assert_known_trio(results: &[(String, Status)], runner: &str) {
    assert_eq!(
        results,
        &[
            ("Add an item to the list".to_owned(), Status::Passed),
            (
                "Adding one item yields two items".to_owned(),
                Status::Failed
            ),
            ("Remove an item from the list".to_owned(), Status::Undefined),
        ],
        "unexpected {runner} status trio"
    );
}

#[test]
fn pytest_bdd_fixture_end_to_end() {
    let _guard = PYTHON_TODO.lock().expect("fixture lock");
    let dir = fixture_project("python-todo");
    let report = verify::run(&dir, &Selection::All).expect("python fixture verify runs");

    assert_eq!(report.outcome, Outcome::Failed);
    assert_eq!(report.stacks.len(), 1);
    assert_eq!(report.stacks[0].stack, "python");
    let trio: Vec<(String, Status)> = report.stacks[0]
        .results
        .iter()
        .map(|r| (r.scenario.clone(), r.status))
        .collect();
    assert_known_trio(&trio, "pytest-bdd");

    // The undefined verdict must carry the junit evidence (ADR-002 merge).
    let undefined = &report.stacks[0].results[2];
    assert!(
        undefined
            .failure
            .as_deref()
            .expect("undefined carries junit failure detail")
            .contains("StepDefinitionNotFoundError")
    );
}

/// `bun install --frozen-lockfile` when the fixture's `node_modules` is
/// absent (fresh checkout) — the committed `bun.lock` pins the resolution.
fn ensure_bun_install(dir: &Path) {
    if dir.join("node_modules").is_dir() {
        return;
    }
    let status = std::process::Command::new("bun")
        .args(["install", "--frozen-lockfile"])
        .current_dir(dir)
        .status()
        .expect("spawn bun install");
    assert!(status.success(), "bun install failed in {}", dir.display());
}

#[test]
fn cucumber_js_fixture_end_to_end() {
    let _guard = TS_TODO.lock().expect("fixture lock");
    let dir = fixture_project("ts-todo");
    ensure_bun_install(&dir);
    let report = verify::run(&dir, &Selection::All).expect("ts fixture verify runs");

    assert_eq!(report.outcome, Outcome::Failed);
    assert_eq!(report.stacks.len(), 1);
    assert_eq!(report.stacks[0].stack, "typescript");
    let trio: Vec<(String, Status)> = report.stacks[0]
        .results
        .iter()
        .map(|r| (r.scenario.clone(), r.status))
        .collect();
    assert_known_trio(&trio, "cucumber-js");
}

#[test]
fn cucumber_js_scenario_filter_maps_to_name() {
    let _guard = TS_TODO.lock().expect("fixture lock");
    let dir = fixture_project("ts-todo");
    ensure_bun_install(&dir);
    let report = verify::run(
        &dir,
        &Selection::Scenario("Add an item to the list".to_owned()),
    )
    .expect("filtered ts verify runs");

    assert_eq!(report.outcome, Outcome::Passed);
    let all: Vec<&str> = report.results().map(|r| r.scenario.as_str()).collect();
    assert_eq!(all, vec!["Add an item to the list"]);
}

/// The impact-map coverage capture, end to end on the python fixture: a
/// full verify run must leave a coverage-kind map whose entries point at
/// the real executed files (pytest-cov contexts → scenario names).
#[test]
fn python_full_verify_writes_a_coverage_impact_map() {
    let _guard = PYTHON_TODO.lock().expect("fixture lock");
    let dir = fixture_project("python-todo");
    let map_path = dir.join(".craftsman/cache/impact-map.json");
    let _ = std::fs::remove_file(&map_path);

    verify::run(&dir, &Selection::All).expect("python fixture verify runs");

    let text = std::fs::read_to_string(&map_path).expect("full run writes the impact map");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("map is valid JSON");
    assert_eq!(doc["version"], 2, "{doc:#}");
    let python = &doc["stacks"]["python"];
    assert_eq!(python["kind"], "coverage", "{doc:#}");
    let files = python["scenarios"]["Add an item to the list"]
        .as_array()
        .expect("covered files for the passing scenario");
    assert!(
        files.iter().any(|f| f == "tests/test_todo.py"),
        "coverage must name the executed test module: {files:?}"
    );
}

#[test]
fn pytest_bdd_scenario_filter_maps_to_k() {
    let _guard = PYTHON_TODO.lock().expect("fixture lock");
    let dir = fixture_project("python-todo");
    let report = verify::run(
        &dir,
        &Selection::Scenario("Add an item to the list".to_owned()),
    )
    .expect("filtered python verify runs");

    assert_eq!(report.outcome, Outcome::Passed);
    let all: Vec<&str> = report.results().map(|r| r.scenario.as_str()).collect();
    assert_eq!(all, vec!["Add an item to the list"]);
}

/// Root-cause test for the craftsman-web ledger finding 2: the cucumber-js
/// adapter must never install dependencies in the verdict path. A project
/// whose runner is not installed gets a deterministic refusal naming the
/// missing dev dependency, and the committed lockfile stays byte-identical
/// (the observed defect: `bunx` auto-installed and saved a new bun.lock).
#[test]
fn cucumber_js_missing_runner_refuses_without_installing() {
    let src = fixture_project("ts-todo");
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    for entry in ["craftsman.toml", "package.json", "bun.lock"] {
        std::fs::copy(src.join(entry), dir.join(entry)).expect("copy fixture file");
    }
    for rel in [
        "features/todo.feature",
        "features/step_definitions/steps.mjs",
        "src/calc.ts",
    ] {
        let dst = dir.join(rel);
        std::fs::create_dir_all(dst.parent().expect("parent")).expect("mkdir");
        std::fs::copy(src.join(rel), dst).expect("copy fixture file");
    }
    let lock_before = std::fs::read(dir.join("bun.lock")).expect("lockfile");

    let err = verify::run(dir, &Selection::All).expect_err("no runner installed must refuse");

    let msg = format!("{err:#}");
    assert!(
        msg.contains("@cucumber/cucumber"),
        "refusal must name the missing dev dependency: {msg}"
    );
    assert!(
        msg.contains("never installs"),
        "must be the deterministic preflight refusal, not a runner artifact \
         error after executing registry code: {msg}"
    );
    let lock_after = std::fs::read(dir.join("bun.lock")).expect("lockfile survives");
    assert_eq!(lock_before, lock_after, "verdict path mutated the lockfile");
    assert!(
        !dir.join("node_modules").exists(),
        "verdict path installed dependencies"
    );
}

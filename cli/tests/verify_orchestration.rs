//! GAP-R02 / GAP-R03 / GAP-R10 characterization pins over the cached
//! green cucumber fixture: impact's computed-empty honesty, check-all's
//! --changed orchestration mapping, batch plan-drift warnings, and the
//! recorded-run replacement semantics of a filtered verify.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;

/// One fixture, serialized: every test shares its cached `target/`.
static FIXTURE: Mutex<()> = Mutex::new(());

const TWO_SCENARIO_SPEC: &str = "Feature: Orchestration fixture\n\n  Scenario: The loop closes\n    Given a truth\n    Then it holds\n\n  Scenario: The loop closes again\n    Given a truth\n    Then it holds\n";

fn fixture() -> PathBuf {
    let dir = std::env::temp_dir().join("craftsman-orchestration-fixture");
    craftsman::doctor::scaffold_rust_fixture(&dir, TWO_SCENARIO_SPEC, true).expect("scaffold");
    // The scaffold enables only verify; the check-all orchestration pin
    // needs a lint gate to observe the --changed narrowing note.
    std::fs::write(
        dir.join("craftsman.toml"),
        "[project]\nname = \"doctor-fixture\"\nstacks = [\"rust\"]\n\n[gates]\nverify = \"strict\"\nlint = \"strict\"\n",
    )
    .expect("config");
    let _ = std::fs::remove_dir_all(dir.join(".craftsman"));
    let _ = std::fs::remove_dir_all(dir.join(".git"));
    std::fs::write(dir.join(".gitignore"), "target/\n.craftsman/\nCargo.lock\n")
        .expect("gitignore");
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["add", "-A"]);
    let identity = [
        "-c",
        "user.name=fixture",
        "-c",
        "user.email=f@example.invalid",
    ];
    let commit: Vec<&str> = identity
        .into_iter()
        .chain(["commit", "--quiet", "-m", "init"])
        .collect();
    git(&dir, &commit);
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("spawn git");
    assert!(status.success(), "git {args:?}");
}

fn craftsman(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_craftsman"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn craftsman")
}

fn assert_exit(output: &Output, expected: i32) -> (String, String) {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(
        output.status.code(),
        Some(expected),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    (stdout, stderr)
}

/// GAP-R02 pin: an impact selection that computes to the empty set —
/// clean tree, nothing changed — exits 0 with the loud nothing-to-run
/// note, distinct from exit 4 (which means the *requested* selection
/// matched nothing).
#[test]
fn impact_computed_empty_exits_zero_with_a_loud_note() {
    let _guard = FIXTURE.lock().expect("fixture lock");
    let dir = fixture();
    assert_exit(&craftsman(&dir, &["verify"]), 0);

    let (_, stderr) = assert_exit(&craftsman(&dir, &["verify", "--impact"]), 0);
    assert!(
        stderr.contains("nothing to run"),
        "the empty computation must be announced:\n{stderr}"
    );
}

/// GAP-R03 pin (orchestration): `check-all --changed` maps verify onto
/// the impact selection and narrows lint per stack — both visible as
/// notes on a clean tree.
#[test]
fn check_all_changed_maps_verify_to_impact_and_narrows_lint() {
    let _guard = FIXTURE.lock().expect("fixture lock");
    let dir = fixture();
    assert_exit(&craftsman(&dir, &["verify"]), 0);

    let (_, stderr) = assert_exit(&craftsman(&dir, &["check-all", "--changed"]), 0);
    assert!(
        stderr.contains("impact:"),
        "verify must run under the impact selection:\n{stderr}"
    );
    assert!(
        stderr.contains("no changed files — tools skipped"),
        "lint must be narrowed to the (empty) changed set:\n{stderr}"
    );
}

/// GAP-R03 pin (batch drift): `verify --batch N` warns about scenarios
/// the plan lists but the spec lacks, and still runs the found subset.
#[test]
fn verify_batch_warns_on_plan_drift_and_runs_the_found_subset() {
    let _guard = FIXTURE.lock().expect("fixture lock");
    let dir = fixture();
    std::fs::write(
        dir.join("PLAN.md"),
        "# Plan\n\n## Batch 1\n\nScenarios:\n- The loop closes\n- A ghost scenario\n",
    )
    .expect("plan");

    let (_, stderr) = assert_exit(&craftsman(&dir, &["verify", "--batch", "1"]), 0);
    std::fs::remove_file(dir.join("PLAN.md")).expect("cleanup plan");
    assert!(
        stderr.contains("plan drift") && stderr.contains("A ghost scenario"),
        "drift must be warned, naming the missing scenario:\n{stderr}"
    );
    assert!(
        stderr.contains("1 passed") || stderr.contains("The loop closes"),
        "the found subset must still run:\n{stderr}"
    );
}

/// GAP-R10 pin (observed live 2026-07-18): a filtered verify run replaces
/// the entire recorded run — after `verify --scenario X`, spec status
/// reports every other scenario unknown until a full run re-records them.
/// Pinned as-is; whether records should merge per scenario is a separate
/// decision.
#[test]
fn filtered_verify_replaces_the_whole_recorded_run() {
    let _guard = FIXTURE.lock().expect("fixture lock");
    let dir = fixture();
    assert_exit(&craftsman(&dir, &["verify"]), 0);
    let (stdout, _) = assert_exit(&craftsman(&dir, &["spec", "status", "--json"]), 0);
    let doc: serde_json::Value = serde_json::from_str(&stdout).expect("status JSON");
    let statuses = |doc: &serde_json::Value| -> Vec<(String, String)> {
        doc["scenarios"]
            .as_array()
            .expect("scenarios")
            .iter()
            .map(|s| {
                (
                    s["scenario"].as_str().unwrap_or_default().to_owned(),
                    s["status"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect()
    };
    assert!(
        statuses(&doc).iter().all(|(_, st)| st == "passed"),
        "full run records everything: {doc:#}"
    );

    assert_exit(
        &craftsman(&dir, &["verify", "--scenario", "The loop closes"]),
        0,
    );
    let (stdout, _) = assert_exit(&craftsman(&dir, &["spec", "status", "--json"]), 0);
    let doc: serde_json::Value = serde_json::from_str(&stdout).expect("status JSON");
    let after = statuses(&doc);
    assert!(
        after
            .iter()
            .any(|(name, st)| name == "The loop closes" && st == "passed"),
        "{after:?}"
    );
    assert!(
        after
            .iter()
            .any(|(name, st)| name == "The loop closes again" && st == "unknown"),
        "the filtered run replaces the record — the other scenario is \
         forgotten (current behavior, pinned): {after:?}"
    );
}

//! Step definitions — the recovered spec-engine and plan scenarios
//! (Batch 11): staleness notes, regex-hostile-name warnings, code-gen
//! empty selection, the write-once a11y stub, and plan lint findings.

use std::process::Command;

use cucumber::{given, then, when};

use crate::{CliWorld, MINIMAL_CONFIG};

#[given("a craftsman project with a recorded green verify run")]
fn project_with_recorded_run(w: &mut CliWorld) {
    crate::project_steps::scaffold_green_fixture(w, "craftsman-spec-stale-fixture");
    // Staleness is HEAD movement, so the fixture needs a repository with
    // a commit recorded before the verify run.
    let dir = w.project_dir();
    let _ = std::fs::remove_dir_all(dir.join(".git"));
    std::fs::write(dir.join(".gitignore"), "target/\n.craftsman/\nCargo.lock\n")
        .expect("write .gitignore");
    crate::repo_steps::git_init_commit_all(&dir);
    w.run_craftsman(&["verify"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "priming verify must pass:\n{}",
        w.combined_output()
    );
}

#[given("a commit has moved the repository head since that run")]
fn commit_moves_head(w: &mut CliWorld) {
    let dir = w.project_dir();
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "move the head",
        ])
        .current_dir(&dir)
        .status()
        .expect("spawn git commit");
    assert!(status.success(), "git commit failed in {}", dir.display());
}

#[given(expr = "a craftsman project whose spec has a scenario named {string}")]
fn project_with_named_scenario(w: &mut CliWorld, name: String) {
    w.write("craftsman.toml", MINIMAL_CONFIG);
    w.write(
        "SPEC.md",
        &format!("Feature: Fixture feature\n\n  Scenario: {name}\n"),
    );
}

#[then("the output contains a warning about the scenario name")]
fn warning_about_scenario_name(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("regex metacharacter"),
        "no regex-metacharacter warning in:\n{combined}"
    );
}

#[given(expr = "a craftsman project configured with only the stack {string}")]
fn project_with_single_stack(w: &mut CliWorld, stack: String) {
    w.write(
        "craftsman.toml",
        &format!("[project]\nname = \"fixture\"\nstacks = [\"{stack}\"]\n"),
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
}

/// The write-once audit stub `spec gen --a11y-stub` emits at the root.
const A11Y_STUB: &str = "AccessibilityAuditTests.swift.template";
const A11Y_HAND_EDIT: &str = "// hand-tuned audit — do not lose me\n";

#[given("a swift-stack craftsman project where the a11y stub was generated and then hand-edited")]
fn swift_project_with_edited_stub(w: &mut CliWorld) {
    w.write(
        "craftsman.toml",
        "[project]\nname = \"fixture\"\nstacks = [\"swift\"]\n",
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    w.run_craftsman(&["spec", "gen", "--a11y-stub"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "priming spec gen --a11y-stub must pass:\n{}",
        w.combined_output()
    );
    assert!(
        w.project_dir().join(A11Y_STUB).is_file(),
        "the stub must exist after generation"
    );
    w.write(A11Y_STUB, A11Y_HAND_EDIT);
}

#[when("the a11y stub generation runs again")]
fn a11y_stub_runs_again(w: &mut CliWorld) {
    w.run_craftsman(&["spec", "gen", "--a11y-stub"]);
}

#[then(expr = "the stub file reports {string}")]
fn stub_file_reports(w: &mut CliWorld, action: String) {
    let combined = w.combined_output();
    assert!(
        combined.contains(&action),
        "stub report lacks {action:?}:\n{combined}"
    );
}

#[then("the hand edit is preserved")]
fn hand_edit_preserved(w: &mut CliWorld) {
    let text = std::fs::read_to_string(w.project_dir().join(A11Y_STUB)).expect("read stub");
    assert_eq!(text, A11Y_HAND_EDIT, "the write-once stub was overwritten");
}

#[given(expr = "its plan assigns the scenario {string} to batch {int} and to batch {int}")]
fn plan_assigns_twice(w: &mut CliWorld, name: String, first: u32, second: u32) {
    w.write(
        "PLAN.md",
        &format!(
            "# Plan\n\n## Batch {first}\n\nScenarios:\n- {name}\n\n## Batch {second}\n\nScenarios:\n- {name}\n"
        ),
    );
}

#[given(expr = "its plan assigns batch {int} only the scenario {string}")]
fn plan_assigns_only(w: &mut CliWorld, batch: u32, name: String) {
    w.write(
        "PLAN.md",
        &format!("# Plan\n\n## Batch {batch}\n\nScenarios:\n- {name}\n"),
    );
}

#[then(expr = "the output warns about {string}")]
fn output_warns_about(w: &mut CliWorld, name: String) {
    let combined = w.combined_output();
    assert!(
        combined.contains(&name),
        "no warning naming {name:?} in:\n{combined}"
    );
    assert!(
        combined.contains("unassigned") || combined.contains("warning"),
        "output has no warning wording:\n{combined}"
    );
}

//! Step definitions — delta mediation (Batch 18): `spec lint --delta`,
//! `spec merge-delta`, and plan lint's delta awareness.

use cucumber::{given, then};

use crate::{CliWorld, fixtures};

/// The delta file sits next to the executed spec. The fixture becomes a
/// single-commit repository so head-unchanged assertions have a head to
/// remember — `spec merge-delta` writes the spec but never commits.
#[given(expr = "a delta file adds the scenario {string}")]
fn delta_file_adds_scenario(w: &mut CliWorld, name: String) {
    w.write(
        "SPEC.delta.md",
        &format!("Feature: Fixture feature — delta\n\n  Scenario: {name}\n"),
    );
    let dir = w.project_dir();
    if !dir.join(".git").exists() {
        fixtures::git_init_commit_all(&dir);
    }
    w.remembered_head = Some(
        fixtures::git_stdout(&dir, &["rev-parse", "HEAD"])
            .trim()
            .to_owned(),
    );
}

#[given(expr = "the plan assigns {string} to a batch")]
fn plan_assigns_to_a_batch(w: &mut CliWorld, name: String) {
    w.write(
        "PLAN.md",
        &format!("# Plan\n\n## Batch 1\n\nScenarios:\n- {name}\n"),
    );
}

#[then("the delta file is gone")]
fn delta_file_gone(w: &mut CliWorld) {
    let path = w.project_dir().join("SPEC.delta.md");
    assert!(!path.exists(), "{} survived the merge", path.display());
}

#[then("the delta file still exists")]
fn delta_file_still_exists(w: &mut CliWorld) {
    let path = w.project_dir().join("SPEC.delta.md");
    assert!(
        path.is_file(),
        "{} was removed by a refusal",
        path.display()
    );
}

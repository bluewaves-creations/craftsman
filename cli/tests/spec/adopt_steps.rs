//! Step definitions — the recovered adopt-phase and session/decision
//! scenarios (Batch 11): phase sequencing, phase-1 scaffolds, extract
//! accumulation, and adr index regeneration.

use cucumber::{given, then};

use crate::{CliWorld, MINIMAL_CONFIG, fixtures};

#[given("adoption phase 0 has been started")]
fn adopt_phase_0_started(w: &mut CliWorld) {
    w.prime(&["adopt", "--start-phase", "0"]);
}

#[given("adoption phase 0 has been started and completed")]
fn adopt_phase_0_done(w: &mut CliWorld) {
    w.prime(&["adopt", "--start-phase", "0"]);
    w.prime(&["adopt", "--complete-phase", "0"]);
}

#[given(expr = "a git repository with a hand-written craftsman.toml naming the project {string}")]
fn repo_with_hand_written_config(w: &mut CliWorld, name: String) {
    let dir = w.project_dir();
    fixtures::git(&dir, &["init", "--quiet"]);
    w.write(
        "craftsman.toml",
        &format!("[project]\nname = \"{name}\"\nstacks = [\"rust\"]\n"),
    );
}

#[then(expr = "the config still names the project {string}")]
fn config_still_names_project(w: &mut CliWorld, name: String) {
    let text = std::fs::read_to_string(w.project_dir().join("craftsman.toml"))
        .expect("read craftsman.toml");
    assert!(
        text.contains(&format!("name = \"{name}\"")),
        "craftsman.toml no longer names {name:?}:\n{text}"
    );
}

/// Config + spec for the session scenarios, then one extract per batch.
fn extract_failed_approach(w: &mut CliWorld, batch: u32, approach: &str) {
    w.write("craftsman.toml", MINIMAL_CONFIG);
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    w.prime(&[
        "extract",
        "--batch",
        &batch.to_string(),
        "--failed",
        approach,
    ]);
}

#[given(expr = "a craftsman project where batch {int} extracted the failed approach {string}")]
fn project_with_batch_extract(w: &mut CliWorld, batch: u32, approach: String) {
    extract_failed_approach(w, batch, &approach);
}

#[given(expr = "batch {int} extracted the failed approach {string}")]
fn another_batch_extract(w: &mut CliWorld, batch: u32, approach: String) {
    extract_failed_approach(w, batch, &approach);
}

#[then(expr = "the learnings record contains both {string} and {string}")]
fn learnings_record_contains(w: &mut CliWorld, first: String, second: String) {
    let path = w.project_dir().join(".craftsman/session/learnings.md");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    for needle in [&first, &second] {
        assert!(
            text.contains(needle.as_str()),
            "learnings.md lacks {needle:?}:\n{text}"
        );
    }
}

#[given("a craftsman project where no extract has ever run")]
fn project_without_extracts(w: &mut CliWorld) {
    w.write("craftsman.toml", MINIMAL_CONFIG);
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
}

#[given(expr = "a craftsman project with the decision {string} and a previously generated index")]
fn project_with_decision_and_stale_index(w: &mut CliWorld, title: String) {
    w.write("craftsman.toml", MINIMAL_CONFIG);
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    std::fs::create_dir_all(w.project_dir().join("decisions")).expect("mkdirs");
    w.write(
        "decisions/ADR-001-alpha-choice.md",
        &format!("# {title}\n\nStatus: accepted · Date: 2026-07-18\n\nBody.\n"),
    );
    // A stale index from an earlier generation: regeneration must replace
    // it and never count it as a decision of its own.
    w.write(
        "decisions/index.md",
        "# Decisions\n\n- stale line from a previous generation\n",
    );
}

#[then(expr = "the regenerated index lists exactly {int} decision")]
fn regenerated_index_lists(w: &mut CliWorld, count: usize) {
    let path = w.project_dir().join("decisions/index.md");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let decisions = text.lines().filter(|l| l.contains("ADR-")).count();
    assert_eq!(
        decisions, count,
        "expected {count} decision line(s) in decisions/index.md:\n{text}"
    );
}

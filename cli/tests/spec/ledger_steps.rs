//! Step definitions — the recovered ledger scenarios (Batch 11): the
//! green-gates trailer, the red-gate refusal, and the forged-trailer
//! rejection.

use cucumber::{given, then, when};

use crate::{CliWorld, fixtures};

/// A committed green fixture: the doctor scaffold under a fresh
/// single-commit repository (its gates all pass).
fn committed_fixture(w: &mut CliWorld, dir_name: &str) {
    crate::project_steps::scaffold_green_fixture(w, dir_name);
    let dir = w.project_dir();
    w.remembered_head = Some(fixtures::recommit_scaffold(&dir));
}

#[given("a craftsman project whose gates are all green")]
fn green_gates_project(w: &mut CliWorld) {
    // More than one scenario shares this Given; a per-invocation directory
    // keeps them collision-free under cucumber's concurrent runner (the
    // scaffold scrubs .git on entry — sharing a live dir would race).
    static INVOCATION: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = INVOCATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    committed_fixture(w, &format!("craftsman-spec-ledger-green-fixture-{n}"));
}

/// Same scaffold plus a scenario whose step has no definition — verify
/// reports it undefined, which is a red gate (never a silent pass).
#[given("a craftsman project whose verify gate is red")]
fn red_verify_project(w: &mut CliWorld) {
    let dir = fixtures::stable_dir("craftsman-spec-ledger-red-fixture");
    let spec = "Feature: Scaffold fixture\n\n  Scenario: The loop closes\n    Given a truth\n    Then it holds\n\n  Scenario: Something not yet written\n    Given an unwritten step\n";
    craftsman::doctor::scaffold_rust_fixture(&dir, spec, true)
        .unwrap_or_else(|e| panic!("scaffold red fixture: {e}"));
    fixtures::scrub(&dir, &[".craftsman"]);
    w.remembered_head = Some(fixtures::recommit_scaffold(&dir));
    w.fixed_dir = Some(dir);
}

#[given("a file is staged")]
fn a_file_is_staged(w: &mut CliWorld) {
    let dir = w.project_dir();
    if !dir.join(".git").exists() {
        fixtures::git(&dir, &["init", "--quiet"]);
    }
    w.write("NOTES.md", "# staged fixture file\n");
    fixtures::git(&dir, &["add", "NOTES.md"]);
}

#[when(expr = "I run craftsman commit with type {string} and message {string}")]
fn run_commit_with(w: &mut CliWorld, commit_type: String, message: String) {
    w.run_craftsman(&["commit", "--type", &commit_type, "--message", &message]);
}

#[when(expr = "I run craftsman commit with a learned line containing {string}")]
fn run_commit_with_learned(w: &mut CliWorld, learned: String) {
    w.run_craftsman(&[
        "commit",
        "--type",
        "fix",
        "--message",
        "sneak",
        "--learned",
        &learned,
    ]);
}

#[then("the new commit message carries a Verified-by trailer naming the gates that ran")]
fn new_commit_carries_trailer(w: &mut CliWorld) {
    let dir = w.project_dir();
    let body = fixtures::git_stdout(&dir, &["log", "-1", "--format=%B"]);
    assert!(
        body.contains("Verified-by:") && body.contains("verify"),
        "the new commit body lacks the CLI-written trailer:\n{body}"
    );
}

#[then("the repository head is unchanged")]
fn head_unchanged(w: &mut CliWorld) {
    let dir = w.project_dir();
    let now = fixtures::git_stdout(&dir, &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    let before = w.remembered_head.clone().expect("a remembered head");
    assert_eq!(now, before, "the refused commit moved the head");
}

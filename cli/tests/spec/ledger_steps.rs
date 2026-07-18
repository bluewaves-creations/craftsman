//! Step definitions — the recovered ledger scenarios (Batch 11): the
//! green-gates trailer, the red-gate refusal, and the forged-trailer
//! rejection.

use std::process::Command;

use cucumber::{given, then, when};

use crate::CliWorld;

fn git_stdout(dir: &std::path::Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn git");
    assert!(out.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A committed green fixture: the doctor scaffold under a fresh
/// single-commit repository (its gates all pass).
fn committed_fixture(w: &mut CliWorld, dir_name: &str) {
    crate::project_steps::scaffold_green_fixture(w, dir_name);
    let dir = w.project_dir();
    // A previous run's staged file must not ride into the initial commit,
    // or staging the identical file again stages nothing.
    let _ = std::fs::remove_file(dir.join("NOTES.md"));
    let _ = std::fs::remove_dir_all(dir.join(".git"));
    std::fs::write(dir.join(".gitignore"), "target/\n.craftsman/\nCargo.lock\n")
        .expect("write .gitignore");
    crate::repo_steps::git_init_commit_all(&dir);
    w.remembered_head = Some(git_stdout(&dir, &["rev-parse", "HEAD"]).trim().to_owned());
}

#[given("a craftsman project whose gates are all green")]
fn green_gates_project(w: &mut CliWorld) {
    committed_fixture(w, "craftsman-spec-ledger-green-fixture");
}

/// Same scaffold plus a scenario whose step has no definition — verify
/// reports it undefined, which is a red gate (never a silent pass).
#[given("a craftsman project whose verify gate is red")]
fn red_verify_project(w: &mut CliWorld) {
    let dir = std::env::temp_dir().join("craftsman-spec-ledger-red-fixture");
    let spec = "Feature: Scaffold fixture\n\n  Scenario: The loop closes\n    Given a truth\n    Then it holds\n\n  Scenario: Something not yet written\n    Given an unwritten step\n";
    craftsman::doctor::scaffold_rust_fixture(&dir, spec, true)
        .unwrap_or_else(|e| panic!("scaffold red fixture: {e}"));
    let _ = std::fs::remove_dir_all(dir.join(".craftsman"));
    let _ = std::fs::remove_file(dir.join("NOTES.md"));
    let _ = std::fs::remove_dir_all(dir.join(".git"));
    std::fs::write(dir.join(".gitignore"), "target/\n.craftsman/\nCargo.lock\n")
        .expect("write .gitignore");
    crate::repo_steps::git_init_commit_all(&dir);
    w.remembered_head = Some(git_stdout(&dir, &["rev-parse", "HEAD"]).trim().to_owned());
    w.fixed_dir = Some(dir);
}

#[given("a file is staged")]
fn a_file_is_staged(w: &mut CliWorld) {
    let dir = w.project_dir();
    if !dir.join(".git").exists() {
        let status = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&dir)
            .status()
            .expect("spawn git init");
        assert!(status.success(), "git init failed in {}", dir.display());
    }
    w.write("NOTES.md", "# staged fixture file\n");
    let status = Command::new("git")
        .args(["add", "NOTES.md"])
        .current_dir(&dir)
        .status()
        .expect("spawn git add");
    assert!(status.success(), "git add failed in {}", dir.display());
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
    let body = git_stdout(&dir, &["log", "-1", "--format=%B"]);
    assert!(
        body.contains("Verified-by:") && body.contains("verify"),
        "the new commit body lacks the CLI-written trailer:\n{body}"
    );
}

#[then("the repository head is unchanged")]
fn head_unchanged(w: &mut CliWorld) {
    let dir = w.project_dir();
    let now = git_stdout(&dir, &["rev-parse", "HEAD"]).trim().to_owned();
    let before = w.remembered_head.clone().expect("a remembered head");
    assert_eq!(now, before, "the refused commit moved the head");
}

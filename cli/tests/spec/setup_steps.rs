//! Step definitions — the recovered setup scenarios (Batch 11):
//! idempotent second run, hand-modified trees left/replaced, and the
//! remove mirror of the attribution proofs.

use cucumber::{given, then, when};

use crate::CliWorld;

/// The skill this module hand-modifies and the foreign file it plants.
const MODIFIED_SKILL: &str = "craftsman-fix";
const EXTRA_FILE: &str = "HAND-NOTES.md";

/// A skill setup installs untouched, for the remove proofs.
const UNMODIFIED_SKILL: &str = "craftsman-spec";

fn sandboxed_home_with_setup_run(w: &mut CliWorld) {
    let home = tempfile::tempdir().expect("home tempdir");
    std::fs::create_dir_all(home.path().join(".claude")).expect("claude marker");
    w.home = Some(home);
    let _ = w.project_dir();
    w.prime(&["setup"]);
}

fn plant_hand_written_file(w: &mut CliWorld) {
    let path = w
        .home
        .as_ref()
        .expect("sandboxed home")
        .path()
        .join(".agents/skills")
        .join(MODIFIED_SKILL)
        .join(EXTRA_FILE);
    std::fs::write(&path, "# hand-written notes — not setup's\n")
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

#[given("a sandboxed home directory where craftsman setup has already run")]
fn home_with_setup_run(w: &mut CliWorld) {
    sandboxed_home_with_setup_run(w);
}

#[given("a sandboxed home directory where a canonical skill tree holds an extra hand-written file")]
fn home_with_modified_skill(w: &mut CliWorld) {
    sandboxed_home_with_setup_run(w);
    plant_hand_written_file(w);
}

#[given("a sandboxed home directory where one installed skill tree was hand-modified")]
fn home_with_one_modified_skill(w: &mut CliWorld) {
    sandboxed_home_with_setup_run(w);
    plant_hand_written_file(w);
}

#[when("I run craftsman setup with force against the sandboxed home")]
fn run_setup_force(w: &mut CliWorld) {
    assert!(w.home.is_some(), "a sandboxed home must be prepared first");
    w.run_craftsman(&["setup", "--force"]);
}

#[when("I run craftsman setup remove against the sandboxed home")]
fn run_setup_remove(w: &mut CliWorld) {
    assert!(w.home.is_some(), "a sandboxed home must be prepared first");
    w.run_craftsman(&["setup", "--remove"]);
}

/// The `canonical`-scope rows of the setup report, as (action, rest) pairs.
fn canonical_rows(output: &str) -> Vec<(String, String)> {
    output
        .lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            (parts.next() == Some("canonical"))
                .then(|| {
                    let action = parts.next()?.to_owned();
                    Some((action, parts.collect::<Vec<_>>().join(" ")))
                })
                .flatten()
        })
        .collect()
}

#[then(expr = "every canonical skill row reports {string}")]
fn every_canonical_row_reports(w: &mut CliWorld, action: String) {
    let combined = w.combined_output();
    let rows = canonical_rows(&combined);
    assert!(!rows.is_empty(), "no canonical rows in:\n{combined}");
    for (got, rest) in &rows {
        assert_eq!(
            got, &action,
            "canonical row {rest:?} reports {got:?}, expected {action:?}:\n{combined}"
        );
    }
}

#[then(expr = "the modified skill row reports {string}")]
fn modified_skill_row_reports(w: &mut CliWorld, action: String) {
    let combined = w.combined_output();
    let row = canonical_rows(&combined)
        .into_iter()
        .find(|(_, rest)| rest.contains(MODIFIED_SKILL))
        .unwrap_or_else(|| panic!("no canonical row for {MODIFIED_SKILL}:\n{combined}"));
    assert_eq!(
        row.0, action,
        "row for {MODIFIED_SKILL} reports {:?}, expected {action:?}:\n{combined}",
        row.0
    );
}

fn hand_written_path(w: &CliWorld) -> std::path::PathBuf {
    w.home
        .as_ref()
        .expect("sandboxed home")
        .path()
        .join(".agents/skills")
        .join(MODIFIED_SKILL)
        .join(EXTRA_FILE)
}

#[then("the hand-written file still exists")]
fn hand_written_file_exists(w: &mut CliWorld) {
    let path = hand_written_path(w);
    assert!(path.is_file(), "{} was removed", path.display());
}

#[then("the hand-written file no longer exists")]
fn hand_written_file_gone(w: &mut CliWorld) {
    let path = hand_written_path(w);
    assert!(!path.exists(), "{} survived --force", path.display());
}

#[then("the modified skill tree still exists")]
fn modified_tree_exists(w: &mut CliWorld) {
    let home = w.home.as_ref().expect("sandboxed home").path();
    let dir = home.join(".agents/skills").join(MODIFIED_SKILL);
    assert!(dir.is_dir(), "{} was removed by setup", dir.display());
}

#[then("the unmodified skill trees and their agent links are removed")]
fn unmodified_trees_removed(w: &mut CliWorld) {
    let home = w.home.as_ref().expect("sandboxed home").path();
    let tree = home.join(".agents/skills").join(UNMODIFIED_SKILL);
    assert!(!tree.exists(), "{} was not removed", tree.display());
    let link = home.join(".claude/skills").join(UNMODIFIED_SKILL);
    assert!(
        !link.exists() && !link.is_symlink(),
        "{} link was not removed",
        link.display()
    );
}

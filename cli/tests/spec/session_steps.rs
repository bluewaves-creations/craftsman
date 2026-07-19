//! Step definitions — boundary observability (Batch 19): the extract
//! receipt and the session distance line.
//!
//! "Ledger commits" are commits carrying a `Verified-by:` trailer; the
//! fixtures here write that trailer with plain git on purpose — the
//! scenarios test the *counting*, not the trailer's unforgeability
//! (that lives in the recovered ledger scenarios).

use cucumber::given;

use crate::{CliWorld, fixtures};

/// A committed green fixture: the doctor scaffold under a fresh
/// single-commit repository.
fn committed_project(w: &mut CliWorld, dir_name: &str) {
    crate::project_steps::scaffold_green_fixture(w, dir_name);
    let dir = w.project_dir();
    w.remembered_head = Some(fixtures::recommit_scaffold(&dir));
}

#[given("a craftsman project where an extract just ran at the current head")]
fn extract_at_current_head(w: &mut CliWorld) {
    committed_project(w, "craftsman-spec-session-zero-fixture");
    w.prime(&["extract"]);
}

#[given("a craftsman project where 2 ledger commits landed after the last extract")]
fn ledger_commits_after_extract(w: &mut CliWorld) {
    // This scenario's own files must not survive into the next run's init
    // commit — an identical re-write would stage nothing (the NOTES.md
    // trap, README.md).
    fixtures::scrub(
        &fixtures::stable_dir("craftsman-spec-session-distance-fixture"),
        &["work-1.md", "work-2.md", "prose.md"],
    );
    committed_project(w, "craftsman-spec-session-distance-fixture");
    w.prime(&["extract"]);
    let dir = w.project_dir();
    for n in 1..=2 {
        w.write(&format!("work-{n}.md"), "fixture work\n");
        fixtures::git(&dir, &["add", "."]);
        fixtures::git(
            &dir,
            &[
                "commit",
                "--quiet",
                "-m",
                &format!("feat: fixture work {n}\n\nVerified-by: fixture gate run"),
            ],
        );
    }
    // One trailer-less commit proves the count filters, not just counts.
    w.write("prose.md", "no trailer here\n");
    fixtures::git(&dir, &["add", "."]);
    fixtures::git(&dir, &["commit", "--quiet", "-m", "docs: prose only"]);
}

#[given("an extract ran at the current head")]
fn extract_ran_here(w: &mut CliWorld) {
    w.prime(&["extract"]);
}

//! Step definitions — the ADR-006 entry doctrine: init's non-empty-tree
//! refusal, `craftsman import` scaffolding/audit/QA detection, and the
//! `[gates.qa]` command gates (Batch 16).

use std::process::Command;

use cucumber::{given, then};

use crate::CliWorld;

/// The pre-existing source content the non-destructive checks compare
/// against byte-for-byte.
const ORIGINAL_SOURCE: &str = "fn main() { println!(\"inherited\"); }\n";

fn git_init(dir: &std::path::Path, add_all: bool) {
    let mut runs = vec![vec!["init", "--quiet"]];
    if add_all {
        runs.push(vec!["add", "-A"]);
    }
    for args in runs {
        let status = Command::new("git")
            .args(&args)
            .current_dir(dir)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }
}

#[given("a git repository that already contains source files")]
fn repo_with_existing_sources(w: &mut CliWorld) {
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("src")).expect("mkdirs");
    w.write("src/original.rs", ORIGINAL_SOURCE);
    w.write("notes.txt", "inherited notes\n");
    git_init(&dir, false);
}

#[given("an imported project whose existing code carries a health finding")]
fn imported_project_with_health_debt(w: &mut CliWorld) {
    w.write(
        "craftsman.toml",
        "[project]\nname = \"legacy\"\nstacks = [\"rust\"]\n\n[gates]\nhealth = \"strict\"\n\n[health]\nmax-function-lines = 5\n",
    );
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("src")).expect("mkdirs");
    w.write(
        "src/lib.rs",
        "pub fn inherited() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    let d = 4;\n    let e = 5;\n    let _ = a + b + c + d + e;\n}\n",
    );
    git_init(&dir, true);
}

#[given(expr = "a git repository with a package script named {string}")]
fn repo_with_package_script(w: &mut CliWorld, script: String) {
    w.write(
        "package.json",
        &format!(
            "{{\n  \"name\": \"legacy\",\n  \"scripts\": {{ \"{script}\": \"echo ok\" }}\n}}\n"
        ),
    );
    let dir = w.project_dir();
    git_init(&dir, false);
}

#[then("no scaffold files were written")]
fn no_scaffold_files_written(w: &mut CliWorld) {
    let dir = w.project_dir();
    for rel in ["craftsman.toml", "AGENTS.md", "SPEC.md", "CLAUDE.md"] {
        assert!(
            !dir.join(rel).exists(),
            "{rel} was written despite the refusal"
        );
    }
}

#[then("the existing source files are unchanged")]
fn existing_sources_unchanged(w: &mut CliWorld) {
    let text = std::fs::read_to_string(w.project_dir().join("src/original.rs"))
        .expect("pre-existing source survives");
    assert_eq!(text, ORIGINAL_SOURCE, "import mutated an existing file");
}

#[then("no baseline was recorded")]
fn no_baseline_recorded(w: &mut CliWorld) {
    let baselines = w.project_dir().join(".craftsman/baselines");
    let entries = std::fs::read_dir(&baselines).map_or(0, Iterator::count);
    assert_eq!(entries, 0, "audit must never record a baseline");
}

#[then(expr = "the output lists {string} as a conversion candidate")]
fn output_lists_conversion_candidate(w: &mut CliWorld, name: String) {
    let combined = w.combined_output();
    let line = combined
        .lines()
        .find(|l| l.contains("QA command candidates"))
        .unwrap_or_else(|| panic!("no QA candidates line in output:\n{combined}"));
    assert!(
        line.contains(&name),
        "{name:?} not listed as a candidate: {line}"
    );
}

fn qa_gate_project(w: &mut CliWorld, command: &str, with_git: bool) {
    w.write(
        "craftsman.toml",
        &format!(
            "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates.qa.smoke]\ncommand = \"{command}\"\n"
        ),
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    if with_git {
        let dir = w.project_dir();
        git_init(&dir, true);
        for args in [
            ["config", "user.name", "fixture"],
            ["config", "user.email", "fixture@example.invalid"],
        ] {
            let status = Command::new("git")
                .args(args)
                .current_dir(&dir)
                .status()
                .expect("spawn git config");
            assert!(status.success());
        }
    }
}

#[given("a craftsman project declaring a qa gate whose command succeeds")]
fn qa_gate_green_project(w: &mut CliWorld) {
    qa_gate_project(w, "true", false);
}

#[given("a craftsman project declaring a qa gate whose command fails")]
fn qa_gate_red_project(w: &mut CliWorld) {
    qa_gate_project(w, "false", true);
}

#[given("a craftsman project declaring a qa gate whose command does not exist")]
fn qa_gate_missing_command_project(w: &mut CliWorld) {
    qa_gate_project(w, "craftsman-definitely-missing-xyz", false);
}

#[then("no commit was created")]
fn no_commit_created(w: &mut CliWorld) {
    let ok = Command::new("git")
        .args(["rev-parse", "--verify", "-q", "HEAD"])
        .current_dir(w.project_dir())
        .status()
        .expect("spawn git")
        .success();
    assert!(!ok, "a commit exists despite the red qa gate");
}

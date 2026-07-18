//! Step definitions — project-level fixtures: spec/plan/config scaffolds,
//! code-gen fixtures, gate fixtures, and their assertions.

use std::process::Command;

use cucumber::{given, then, when};

use crate::{CliWorld, MINIMAL_CONFIG};

#[given(expr = "a craftsman project whose spec has scenarios {string} and {string}")]
fn project_with_two_scenarios(w: &mut CliWorld, first: String, second: String) {
    w.write("craftsman.toml", MINIMAL_CONFIG);
    w.write(
        "SPEC.md",
        &format!("Feature: Fixture feature\n\n  Scenario: {first}\n\n  Scenario: {second}\n"),
    );
}

#[given(expr = "a craftsman project configured with stacks {string} and {string}")]
fn project_with_stacks(w: &mut CliWorld, first: String, second: String) {
    w.write(
        "craftsman.toml",
        &format!("[project]\nname = \"fixture\"\nstacks = [\"{first}\", \"{second}\"]\n"),
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
}

#[given(expr = "a bash-stack craftsman project whose spec has scenarios {string} and {string}")]
fn bash_project_with_two_scenarios(w: &mut CliWorld, first: String, second: String) {
    w.write(
        "craftsman.toml",
        "[project]\nname = \"fixture\"\nstacks = [\"bash\"]\n",
    );
    w.write(
        "SPEC.md",
        &format!("Feature: Fixture feature\n\n  Scenario: {first}\n\n  Scenario: {second}\n"),
    );
}

/// The sentinel line planted into the generated step template to prove gen
/// never overwrites a step file once it exists.
const HAND_MODIFICATION: &str = "# hand-tuned: do not lose me\n";

#[given("spec gen has run and the step template was hand-modified")]
fn spec_gen_ran_and_template_modified(w: &mut CliWorld) {
    w.run_craftsman(&["spec", "gen"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "priming spec gen must pass:\n{}",
        w.combined_output()
    );
    let path = w.project_dir().join("tests/steps.bash.template");
    assert!(path.is_file(), "gen must have created the template");
    std::fs::write(&path, HAND_MODIFICATION)
        .unwrap_or_else(|e| panic!("modify {}: {e}", path.display()));
}

#[then(expr = "the generated bats file contains {string}")]
fn generated_bats_contains(w: &mut CliWorld, needle: String) {
    let path = w.project_dir().join("tests/generated_spec.bats");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(
        text.contains(&needle),
        "{} lacks {needle:?}:\n{text}",
        path.display()
    );
}

#[then("the step template still carries the hand modification")]
fn template_survived(w: &mut CliWorld) {
    let path = w.project_dir().join("tests/steps.bash.template");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert_eq!(
        text, HAND_MODIFICATION,
        "gen must never overwrite an existing step template"
    );
}

#[given(expr = "a craftsman project whose spec has a scenario tagged {string}")]
fn project_with_tagged_scenario(w: &mut CliWorld, tag: String) {
    w.write("craftsman.toml", MINIMAL_CONFIG);
    w.write(
        "SPEC.md",
        &format!("Feature: Fixture feature\n\n  {tag}\n  Scenario: Tagged behavior\n"),
    );
}

#[given(expr = "a craftsman project whose config sets the verify gate to {string}")]
fn project_with_verify_gate(w: &mut CliWorld, mode: String) {
    w.write(
        "craftsman.toml",
        &format!("{MINIMAL_CONFIG}\n[gates]\nverify = \"{mode}\"\n"),
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
}

#[given(expr = "its plan assigns batch {int} the scenarios {string} and {string}")]
fn plan_assigns_batch(w: &mut CliWorld, batch: u32, first: String, second: String) {
    w.write(
        "PLAN.md",
        &format!("# Plan\n\n## Batch {batch}\n\nScenarios:\n- {first}\n- {second}\n"),
    );
}

/// The doctor-fixture spec, green: both steps are implemented.
const GREEN_FIXTURE_SPEC: &str = "Feature: Scaffold fixture\n\n  Scenario: The loop closes\n    Given a truth\n    Then it holds\n";

/// Scaffold a cached rust cucumber fixture (doctor's mechanism) at a
/// stable temp path — each caller gets its own directory so concurrently
/// running scenarios never share a fixture.
fn scaffold_fixture(w: &mut CliWorld, dir_name: &str, spec: &str) {
    let dir = std::env::temp_dir().join(dir_name);
    craftsman::doctor::scaffold_rust_fixture(&dir, spec, true)
        .unwrap_or_else(|e| panic!("scaffold {dir_name}: {e}"));
    // A fresh fixture state: no impact map or other cached CLI state.
    let _ = std::fs::remove_dir_all(dir.join(".craftsman"));
    w.fixed_dir = Some(dir);
}

#[given("a scaffolded rust project that verifies green")]
fn scaffolded_green_project(w: &mut CliWorld) {
    scaffold_fixture(w, "craftsman-spec-impact-fixture", GREEN_FIXTURE_SPEC);
}

/// A green cucumber fixture at `dir_name` for steps outside this module
/// (e.g. the unborn-HEAD first-commit scenario in `repo_steps`).
pub fn scaffold_green_fixture(w: &mut CliWorld, dir_name: &str) {
    scaffold_fixture(w, dir_name, GREEN_FIXTURE_SPEC);
}

#[given("a scaffolded rust project with a recorded green verify run")]
fn scaffolded_recorded_project(w: &mut CliWorld) {
    scaffold_fixture(w, "craftsman-spec-status-fixture", GREEN_FIXTURE_SPEC);
    w.run_craftsman(&["verify"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "priming verify must pass:\n{}",
        w.combined_output()
    );
}

#[given("a scaffolded rust project whose spec has an unimplemented step")]
fn scaffolded_undefined_project(w: &mut CliWorld) {
    // The harness implements only "a truth" / "it holds": the extra step
    // has no step definition, which cucumber-rs reports as an undefined
    // step (ADR-003: step-level skipped in output-json → Undefined).
    let spec = &format!(
        "{GREEN_FIXTURE_SPEC}\n  Scenario: Something not yet written\n    Given an unwritten step\n"
    );
    scaffold_fixture(w, "craftsman-spec-undefined-fixture", spec);
}

/// A minimal cargo library crate with a craftsman config, cached at a
/// stable temp path (its `target/` survives across runs so clippy stays
/// warm). `bad_fmt` seeds one `cargo fmt` finding in `src/lib.rs` line 1;
/// `with_git` makes it a fresh single-commit repository with `target/` and
/// `.craftsman/` ignored (a clean tree, as the gate cache requires).
fn scaffold_gate_fixture(
    w: &mut CliWorld,
    dir_name: &str,
    bad_fmt: bool,
    lint_mode: &str,
    with_git: bool,
) {
    let dir = std::env::temp_dir().join(dir_name);
    std::fs::create_dir_all(dir.join("src")).expect("mkdirs");
    let write = |name: &str, content: &str| {
        std::fs::write(dir.join(name), content)
            .unwrap_or_else(|e| panic!("write {name} in {}: {e}", dir.display()));
    };
    write(
        "Cargo.toml",
        "[package]\nname = \"gatefix\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    write(
        "src/lib.rs",
        if bad_fmt {
            "pub fn f( x:i32)->i32{ x+1 }\n"
        } else {
            "pub fn f(x: i32) -> i32 {\n    x + 1\n}\n"
        },
    );
    write(
        "craftsman.toml",
        &format!(
            "[project]\nname = \"gatefix\"\nstacks = [\"rust\"]\n\n[gates]\nlint = \"{lint_mode}\"\n"
        ),
    );
    // Cargo.lock is ignored deliberately: the first clippy run generates
    // it, and an untracked lockfile appearing mid-run would change the
    // gate-cache key between the first and second check-all.
    write(".gitignore", "target/\n.craftsman/\nCargo.lock\n");
    // Fresh gate state on every scenario run: no stale baselines or caches.
    let _ = std::fs::remove_dir_all(dir.join(".craftsman"));
    let _ = std::fs::remove_dir_all(dir.join(".git"));
    if with_git {
        crate::repo_steps::git_init_commit_all(&dir);
    }
    w.fixed_dir = Some(dir);
}

#[given("a rust gate fixture with a seeded formatting finding")]
fn gate_fixture_with_finding(w: &mut CliWorld) {
    scaffold_gate_fixture(
        w,
        "craftsman-spec-lint-finding-fixture",
        true,
        "strict",
        false,
    );
}

#[given("a rust gate fixture with a seeded finding and the lint gate in baseline mode")]
fn gate_fixture_baseline_mode(w: &mut CliWorld) {
    scaffold_gate_fixture(
        w,
        "craftsman-spec-lint-baseline-fixture",
        true,
        "baseline",
        false,
    );
}

#[given("a second rust gate fixture with a seeded finding and the lint gate in baseline mode")]
fn gate_fixture_baseline_mode_second(w: &mut CliWorld) {
    scaffold_gate_fixture(
        w,
        "craftsman-spec-gate-strict-fixture",
        true,
        "baseline",
        false,
    );
}

#[given("a clean rust gate fixture under git with the lint gate strict")]
fn gate_fixture_clean_git(w: &mut CliWorld) {
    scaffold_gate_fixture(
        w,
        "craftsman-spec-gate-cache-fixture",
        false,
        "strict",
        true,
    );
}

#[given("its lint baseline has been recorded")]
fn lint_baseline_recorded(w: &mut CliWorld) {
    w.run_craftsman(&["gate", "baseline", "lint"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "gate baseline lint must pass:\n{}",
        w.combined_output()
    );
}

#[when("I run craftsman check-all twice")]
fn run_check_all_twice(w: &mut CliWorld) {
    w.run_craftsman(&["check-all"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "priming check-all must pass:\n{}",
        w.combined_output()
    );
    w.run_craftsman(&["check-all"]);
}

/// The ts-todo fixture, copied minus `node_modules` — a typescript project
/// whose declared runner is not installed (craftsman-web ledger finding 2).
#[given("a typescript project that does not have the cucumber-js runner installed")]
fn ts_project_without_runner(w: &mut CliWorld) {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ts-todo");
    let dir = w.project_dir();
    for rel in [
        "craftsman.toml",
        "package.json",
        "bun.lock",
        "features/todo.feature",
        "features/step_definitions/steps.mjs",
        "src/calc.ts",
    ] {
        let dst = dir.join(rel);
        std::fs::create_dir_all(dst.parent().expect("parent")).expect("mkdirs");
        std::fs::copy(src.join(rel), &dst).unwrap_or_else(|e| panic!("copy {rel}: {e}"));
    }
}

#[then("the project lockfile is unchanged")]
fn project_lockfile_unchanged(w: &mut CliWorld) {
    let src =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ts-todo/bun.lock");
    let reference = std::fs::read(src).expect("reference lockfile");
    let actual = std::fs::read(w.project_dir().join("bun.lock")).expect("project lockfile");
    assert_eq!(reference, actual, "the verdict path mutated bun.lock");
}

/// A project pinning a gate tool whose hermetic install is absent — the
/// tools dir is pointed at an empty sandbox via `CRAFTSMAN_TOOLS_DIR`, so
/// this machine's real ~/.craftsman/tools never leaks in (ledger 4).
#[given("a craftsman project that pins a gate tool that does not exist on this machine")]
fn project_with_unresolvable_tool_pin(w: &mut CliWorld) {
    w.write(
        "craftsman.toml",
        "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\nverify = \"strict\"\nsecurity = \"strict\"\n\n[gates.tools]\ngitleaks = \"9.9.9-fixture\"\n",
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    let empty_tools = w.project_dir().join("empty-tools");
    std::fs::create_dir_all(&empty_tools).expect("mkdirs");
    w.env.push((
        "CRAFTSMAN_TOOLS_DIR".to_owned(),
        empty_tools.display().to_string(),
    ));
}

/// Baseline-mode health gate over a tree with one inherited finding and no
/// recorded baseline — the craftsman-web situation before its explicit
/// `gate baseline health` (ledger 4b).
#[given(
    "a craftsman project with a baseline-mode health gate, no recorded baseline, and an existing finding"
)]
fn project_with_unbaselined_health_debt(w: &mut CliWorld) {
    w.write(
        "craftsman.toml",
        "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\nhealth = \"baseline\"\n\n[health]\nmax-function-lines = 5\n",
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("src")).expect("mkdirs");
    w.write(
        "src/lib.rs",
        "pub fn inherited() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    let d = 4;\n    let e = 5;\n    let _ = a + b + c + d + e;\n}\n",
    );
    for args in [&["init", "--quiet"][..], &["add", "-A"][..]] {
        let status = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {args:?} failed");
    }
}

/// A hermetic tools dir carrying the scaffold's pinned release binaries
/// (doctor's gate-tools check tests presence, exactly what a machine that
/// ran its gates once while online would have). Versions match
/// `INIT_CONFIG_TOML`; keep in step when the template pins move.
#[given("the scaffold's pinned gate tools are installed on this machine")]
fn pinned_gate_tools_installed(w: &mut CliWorld) {
    // Under .craftsman/ so init's non-empty-tree detection (ADR-006)
    // does not count the sandbox as inherited source.
    let tools = w.project_dir().join(".craftsman/fixture-tools");
    for pin in ["gitleaks@8.24.0", "osv-scanner@2.4.0"] {
        std::fs::create_dir_all(tools.join(pin)).expect("mkdirs");
    }
    w.env.push((
        "CRAFTSMAN_TOOLS_DIR".to_owned(),
        tools.display().to_string(),
    ));
}

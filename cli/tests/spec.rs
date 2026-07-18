//! Self-hosting acceptance harness: runs the repo-root SPEC.md with
//! cucumber-rs, driving the compiled `craftsman` binary against disposable
//! fixture projects in temp directories.
//!
//! ADR-003 convention (which `craftsman verify` relies on): when the
//! `CRAFTSMAN_JSON` environment variable is set, the harness writes
//! cucumber-json results there; otherwise it runs with the default writer
//! and a non-zero exit on failure (`cargo test --test spec`).

#![expect(
    clippy::needless_pass_by_value,
    reason = "cucumber's step macros pass owned, FromStr-extracted parameters"
)]
#![expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber's step macros require `&mut World` as the first argument"
)]

use std::path::PathBuf;
use std::process::{Command, Output};

use cucumber::{World as _, given, then, when};

const MINIMAL_CONFIG: &str = "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n";

#[derive(Debug, Default, cucumber::World)]
pub struct CliWorld {
    dir: Option<tempfile::TempDir>,
    /// A cached scaffolded fixture at a stable path (its compiled `target/`
    /// survives across runs, like doctor's) instead of a throwaway tempdir.
    fixed_dir: Option<PathBuf>,
    output: Option<Output>,
}

impl CliWorld {
    /// The fixture project directory, created on first use.
    fn project_dir(&mut self) -> PathBuf {
        if let Some(fixed) = &self.fixed_dir {
            return fixed.clone();
        }
        if self.dir.is_none() {
            self.dir = Some(tempfile::tempdir().expect("create fixture tempdir"));
        }
        self.dir
            .as_ref()
            .expect("just created")
            .path()
            .to_path_buf()
    }

    fn write(&mut self, name: &str, content: &str) {
        let path = self.project_dir().join(name);
        std::fs::write(&path, content).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    }

    fn run_craftsman(&mut self, args: &[&str]) {
        let dir = self.project_dir();
        let output = Command::new(env!("CARGO_BIN_EXE_craftsman"))
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("spawn craftsman");
        self.output = Some(output);
    }

    const fn output(&self) -> &Output {
        self.output
            .as_ref()
            .expect("a When step must run craftsman first")
    }

    fn combined_output(&self) -> String {
        let o = self.output();
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        )
    }
}

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
        for args in [
            &["init", "--quiet"][..],
            &["add", "-A"][..],
            &[
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "init",
            ][..],
        ] {
            let status = Command::new("git")
                .args(args)
                .current_dir(&dir)
                .status()
                .expect("spawn git");
            assert!(status.success(), "git {args:?} failed in {}", dir.display());
        }
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

#[given("an empty project directory")]
fn empty_project_directory(w: &mut CliWorld) {
    let _ = w.project_dir();
}

/// `git init` + `git add -A` in the fixture dir (arch and health census
/// tracked files via `git ls-files`; no commit needed).
fn git_init_add(dir: &std::path::Path) {
    for args in [&["init", "--quiet"][..], &["add", "-A"][..]] {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }
}

#[given("a craftsman project with an arch deny rule and a violating import")]
fn arch_violation_fixture(w: &mut CliWorld) {
    w.write(
        "craftsman.toml",
        "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\narch = \"strict\"\n\n[arch]\ndeny = [\"src/a -> src/b\"]\n",
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("src/a")).expect("mkdirs");
    std::fs::create_dir_all(dir.join("src/b")).expect("mkdirs");
    w.write("src/a/mod.rs", "use crate::b::helper;\n");
    w.write("src/b/mod.rs", "pub fn helper() {}\n");
    git_init_add(&dir);
}

#[given("a craftsman project whose source has a function longer than the health limit")]
fn health_long_function_fixture(w: &mut CliWorld) {
    w.write(
        "craftsman.toml",
        "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\nhealth = \"strict\"\n\n[health]\nmax-function-lines = 5\n",
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("src")).expect("mkdirs");
    w.write(
        "src/lib.rs",
        "pub fn sprawling() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    let d = 4;\n    let e = 5;\n    let f = 6;\n    let _ = a + b + c + d + e + f;\n}\n",
    );
    git_init_add(&dir);
}

#[given("the project is a fresh git repository")]
fn fresh_git_repository(w: &mut CliWorld) {
    let dir = w.project_dir();
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&dir)
        .status()
        .expect("spawn git init");
    assert!(status.success(), "git init failed in {}", dir.display());
}

#[when(expr = "I run craftsman with {string}")]
fn run_with_args(w: &mut CliWorld, args: String) {
    let args: Vec<&str> = args.split_whitespace().collect();
    w.run_craftsman(&args);
}

#[when(expr = "I run craftsman verify for the scenario {string}")]
fn run_verify_scenario(w: &mut CliWorld, name: String) {
    w.run_craftsman(&["verify", "--scenario", &name]);
}

#[then(expr = "the exit code is {int}")]
fn exit_code_is(w: &mut CliWorld, expected: i32) {
    let actual = w.output().status.code();
    assert_eq!(
        actual,
        Some(expected),
        "expected exit {expected}, got {actual:?}; output:\n{}",
        w.combined_output()
    );
}

#[then(expr = "the output contains {string}")]
fn output_contains(w: &mut CliWorld, needle: String) {
    let combined = w.combined_output();
    assert!(
        combined.contains(&needle),
        "output does not contain {needle:?}:\n{combined}"
    );
}

#[then(expr = "stdout is valid JSON listing {int} scenarios")]
fn stdout_is_json_with_scenarios(w: &mut CliWorld, count: usize) {
    let stdout = String::from_utf8_lossy(&w.output().stdout);
    let doc: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{stdout}"));
    let scenarios = doc["scenarios"].as_array().expect("a `scenarios` array");
    assert_eq!(scenarios.len(), count, "in:\n{stdout}");
}

#[tokio::main]
async fn main() {
    // Repo-root SPEC.md, one directory above this cargo package. The
    // cucumber parser accepts a direct file path regardless of extension
    // and falls back to CARGO_MANIFEST_DIR-relative resolution.
    let spec = "../SPEC.md";
    if let Ok(path) = std::env::var("CRAFTSMAN_JSON") {
        // craftsman verify is driving: write cucumber-json where told.
        let file = std::fs::File::create(&path).unwrap_or_else(|e| panic!("create {path}: {e}"));
        CliWorld::cucumber()
            .with_writer(cucumber::writer::Json::new(file))
            .run(spec)
            .await;
    } else {
        // Direct `cargo test --test spec`: human output, non-zero on red.
        CliWorld::run(spec).await;
    }
}

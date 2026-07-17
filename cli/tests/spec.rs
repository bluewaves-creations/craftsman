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
    output: Option<Output>,
}

impl CliWorld {
    /// The fixture project directory, created on first use.
    fn project_dir(&mut self) -> PathBuf {
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

#[given("an empty project directory")]
fn empty_project_directory(w: &mut CliWorld) {
    let _ = w.project_dir();
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

//! `craftsman doctor` — prove the loop closes.
//!
//! Five checks: config loads (verify strict), spec lints clean, plan lints
//! clean (when a plan exists), required tools resolve, and THE ROUND TRIP —
//! a disposable rust cucumber fixture project is verified red, flipped, and
//! verified green, proving the adapter observes real failure and real
//! success end-to-end. The fixture lives at a stable path under the system
//! temp dir so its `target/` (the expensive part — cucumber + tokio) is
//! compiled once and reused; the first run may take minutes.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use thiserror::Error;

use crate::config::{Config, GateMode, Loaded};
use crate::plan;
use crate::spec::{self, Severity};
use crate::verify::{self, Outcome, Selection};

/// One doctor check verdict.
#[derive(Debug, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub passed: bool,
    pub detail: String,
}

impl Check {
    const fn pass(name: &'static str, detail: String) -> Self {
        Self {
            name,
            passed: true,
            detail,
        }
    }

    const fn fail(name: &'static str, detail: String) -> Self {
        Self {
            name,
            passed: false,
            detail,
        }
    }
}

#[derive(Debug, Error)]
enum RoundTripError {
    #[error("cannot write fixture file {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("verify errored on the fixture: {0}")]
    Verify(#[from] verify::VerifyError),
    #[error("expected the {phase} fixture to be {expected}, observed {observed}")]
    WrongVerdict {
        phase: &'static str,
        expected: &'static str,
        observed: String,
    },
}

/// Run every doctor check. Never fails as a function — failures are carried
/// in the returned checks (exit 1 at the command layer when any is red).
#[must_use]
pub fn run(cwd: &Path) -> Vec<Check> {
    let mut checks = Vec::new();

    let loaded = check_config(&mut checks, cwd);
    let spec_names = check_spec(&mut checks, loaded.as_ref());
    check_plan(&mut checks, loaded.as_ref(), spec_names.as_deref());
    check_tools(&mut checks);
    check_round_trip(&mut checks);

    checks
}

/// (a) config loads and the verify gate is strict.
fn check_config(checks: &mut Vec<Check>, cwd: &Path) -> Option<Loaded> {
    match Config::load(cwd) {
        Ok(loaded) => {
            let check = if loaded.config.gates.verify == Some(GateMode::Strict) {
                Check::pass(
                    "config",
                    format!(
                        "loaded from {} — verify gate strict",
                        loaded.root.join(crate::config::FILE_NAME).display()
                    ),
                )
            } else {
                Check::fail(
                    "config",
                    "verify gate is absent (off) — set `[gates] verify = \"strict\"`".to_owned(),
                )
            };
            checks.push(check);
            Some(loaded)
        }
        Err(err) => {
            checks.push(Check::fail("config", format!("{err}")));
            None
        }
    }
}

/// (b) spec parses and lints clean. Returns the scenario names for (c).
fn check_spec(checks: &mut Vec<Check>, loaded: Option<&Loaded>) -> Option<Vec<String>> {
    let Some(loaded) = loaded else {
        checks.push(Check::fail(
            "spec",
            "blocked: config did not load".to_owned(),
        ));
        return None;
    };
    let path = loaded.root.join(&loaded.config.project.spec);
    match spec::parse_spec(&path) {
        Ok(feature) => {
            let findings = spec::lint(&feature);
            let errors = findings
                .iter()
                .filter(|f| f.severity == Severity::Error)
                .count();
            let names: Vec<String> = spec::inventory(&feature)
                .into_iter()
                .map(|e| e.scenario)
                .collect();
            if errors == 0 {
                checks.push(Check::pass(
                    "spec",
                    format!(
                        "{} lints clean — {} scenarios, {} warning(s)",
                        loaded.config.project.spec,
                        names.len(),
                        findings.len()
                    ),
                ));
            } else {
                checks.push(Check::fail(
                    "spec",
                    format!(
                        "{} has {errors} lint error(s) — run `craftsman spec lint`",
                        loaded.config.project.spec
                    ),
                ));
            }
            Some(names)
        }
        Err(err) => {
            checks.push(Check::fail("spec", format!("{err}")));
            None
        }
    }
}

/// (c) plan lints clean, when the configured plan file exists.
fn check_plan(checks: &mut Vec<Check>, loaded: Option<&Loaded>, spec_names: Option<&[String]>) {
    let Some(loaded) = loaded else {
        checks.push(Check::fail(
            "plan",
            "blocked: config did not load".to_owned(),
        ));
        return;
    };
    let plan_rel = &loaded.config.project.plan;
    let path = loaded.root.join(plan_rel);
    if !path.is_file() {
        checks.push(Check::pass(
            "plan",
            format!("no plan file at {plan_rel} — skipped"),
        ));
        return;
    }
    let Some(names) = spec_names else {
        checks.push(Check::fail(
            "plan",
            "blocked: spec did not parse".to_owned(),
        ));
        return;
    };
    match plan::parse_plan(&path) {
        Ok(batches) => {
            let findings = plan::lint(&batches, names);
            let errors = findings
                .iter()
                .filter(|f| f.severity == Severity::Error)
                .count();
            if errors == 0 {
                checks.push(Check::pass(
                    "plan",
                    format!(
                        "{plan_rel} lints clean — {} batches, {} warning(s)",
                        batches.len(),
                        findings.len()
                    ),
                ));
            } else {
                checks.push(Check::fail(
                    "plan",
                    format!("{plan_rel} has {errors} lint error(s) — run `craftsman plan lint`"),
                ));
            }
        }
        Err(err) => checks.push(Check::fail("plan", format!("{err}"))),
    }
}

/// (d) required tools resolve: git always; cargo too — even without a rust
/// stack, the round trip's fixture is a rust cucumber project.
fn check_tools(checks: &mut Vec<Check>) {
    let mut versions = Vec::new();
    let mut missing = Vec::new();
    for tool in ["git", "cargo"] {
        match tool_version(tool) {
            Some(v) => versions.push(v),
            None => missing.push(tool),
        }
    }
    if missing.is_empty() {
        checks.push(Check::pass("tools", versions.join(", ")));
    } else {
        checks.push(Check::fail(
            "tools",
            format!("missing: {} — install and re-run", missing.join(", ")),
        ));
    }
}

fn tool_version(tool: &str) -> Option<String> {
    let output = std::process::Command::new(tool)
        .arg("--version")
        .output()
        .ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or(tool)
            .trim()
            .to_owned()
    })
}

/// (e) THE ROUND TRIP: red observed, then green observed, end-to-end.
fn check_round_trip(checks: &mut Vec<Check>) {
    match round_trip() {
        Ok(detail) => checks.push(Check::pass("round-trip", detail)),
        Err(err) => checks.push(Check::fail("round-trip", format!("{err}"))),
    }
}

fn round_trip() -> Result<String, RoundTripError> {
    let dir = std::env::temp_dir().join("craftsman-doctor-fixture");
    write_fixture(&dir)?;

    write_truth(&dir, false)?;
    eprintln!(
        "doctor: round trip — red phase in {} (first run compiles the fixture; may take minutes)",
        dir.display()
    );
    let start = Instant::now();
    let red = verify::run(&dir, &Selection::All)?;
    let red_secs = start.elapsed().as_secs_f32();
    if red.outcome != Outcome::Failed || red.counts.failed == 0 {
        return Err(RoundTripError::WrongVerdict {
            phase: "red",
            expected: "Failed",
            observed: format!("{:?} ({:?})", red.outcome, red.counts),
        });
    }

    write_truth(&dir, true)?;
    eprintln!("doctor: round trip — green phase");
    let start = Instant::now();
    let green = verify::run(&dir, &Selection::All)?;
    let green_secs = start.elapsed().as_secs_f32();
    if green.outcome != Outcome::Passed {
        return Err(RoundTripError::WrongVerdict {
            phase: "green",
            expected: "Passed",
            observed: format!("{:?} ({:?})", green.outcome, green.counts),
        });
    }

    Ok(format!(
        "red observed ({red_secs:.1}s), then green observed ({green_secs:.1}s) — \
         fixture cached at {}",
        dir.display()
    ))
}

/// The minimal rust cucumber fixture: one feature, one scenario, two steps.
/// The `Then` step asserts `fixture::HOLDS`, flipped by [`write_truth`] so
/// only `src/lib.rs` recompiles between the red and green runs.
fn write_fixture(dir: &Path) -> Result<(), RoundTripError> {
    write_if_changed(
        &dir.join("craftsman.toml"),
        "[project]\nname = \"doctor-fixture\"\nstacks = [\"rust\"]\n\n[gates]\nverify = \"strict\"\n",
    )?;
    write_if_changed(
        &dir.join("SPEC.md"),
        "Feature: Doctor fixture\n\n  Scenario: The loop closes\n    Given a truth\n    Then it holds\n",
    )?;
    write_if_changed(
        &dir.join("Cargo.toml"),
        r#"[package]
name = "doctor-fixture"
version = "0.0.0"
edition = "2021"
publish = false

[dev-dependencies]
cucumber = { version = "0.23", features = ["output-json"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }

[[test]]
name = "spec"
harness = false

[workspace]
"#,
    )?;
    write_if_changed(
        &dir.join("tests/spec.rs"),
        r#"//! Doctor-fixture harness per the ADR-003 convention: write
//! cucumber-json to the path in CRAFTSMAN_JSON.
use cucumber::{World as _, given, then};

#[derive(Debug, Default, cucumber::World)]
struct W;

#[given("a truth")]
fn a_truth(_w: &mut W) {}

#[then("it holds")]
fn it_holds(_w: &mut W) {
    assert!(doctor_fixture::HOLDS, "the truth does not hold");
}

#[tokio::main]
async fn main() {
    let path = std::env::var("CRAFTSMAN_JSON").expect("CRAFTSMAN_JSON set by craftsman verify");
    let file = std::fs::File::create(&path).expect("create results file");
    W::cucumber()
        .with_writer(cucumber::writer::Json::new(file))
        .run("SPEC.md")
        .await;
}
"#,
    )
}

fn write_truth(dir: &Path, holds: bool) -> Result<(), RoundTripError> {
    write_if_changed(
        &dir.join("src/lib.rs"),
        &format!("pub const HOLDS: bool = {holds};\n"),
    )
}

/// Write only when content differs, keeping mtimes stable so cargo's
/// fingerprinting reuses the cached build.
fn write_if_changed(path: &Path, content: &str) -> Result<(), RoundTripError> {
    if std::fs::read_to_string(path).is_ok_and(|existing| existing == content) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| RoundTripError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, content).map_err(|source| RoundTripError::Write {
        path: path.to_path_buf(),
        source,
    })
}

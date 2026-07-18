//! Round-trip integration tests for the code-gen stacks (Batch 5): scaffold
//! a real project, run `spec gen`, implement scenario A's steps for real,
//! leave scenario B's steps as stubs, and drive the adapter through
//! `verify::run` — asserting A=Passed / B=Undefined, then breaking A and
//! asserting Failed.
//!
//! The bash round trip is fast (~0.3s) and runs whenever `bats` is on PATH.
//! The swift round trip compiles a `SwiftPM` package: its fixture lives at a
//! stable temp path (doctor's caching pattern — the `.build/` dir survives
//! across runs; content-stable writes keep fingerprints valid). Measured on
//! this machine (Swift 6.3.3, M-series): 2.9s cold green + 0.7s red, 0.7s
//! each warm — far under the plan's 90s ignore-threshold, so it stays
//! unignored and self-skips (loudly) when the toolchain is absent or older
//! than Swift 6.2 (SE-0451 raw identifiers). The JSONL fixtures in
//! `tests/normalize_fixtures.rs` are the always-on fast path.

use std::path::Path;
use std::process::Command;

use craftsman::codegen;
use craftsman::verify::normalize::Status;
use craftsman::verify::{self, Outcome, Selection};

/// Two scenarios: A fully implementable, B left undefined by construction.
const ROUND_TRIP_SPEC: &str = "\
Feature: Round trip

  Scenario: Scenario A passes
    Given a seeded counter
    When the counter is bumped
    Then the counter holds two

  Scenario: Scenario B stays undefined
    Given an unwritten step
";

fn statuses(report: &verify::Report) -> Vec<(String, Status)> {
    report
        .results()
        .map(|r| (r.scenario.clone(), r.status))
        .collect()
}

fn assert_trio_phase(report: &verify::Report, a: Status, b: Status, phase: &str) {
    assert_eq!(
        statuses(report),
        vec![
            ("Scenario A passes".to_owned(), a),
            ("Scenario B stays undefined".to_owned(), b),
        ],
        "{phase}: unexpected statuses"
    );
}

/// Content-stable write (doctor's pattern): unchanged files keep their
/// mtimes so the build caches stay warm.
fn write(path: &Path, content: &str) {
    if std::fs::read_to_string(path).is_ok_and(|existing| existing == content) {
        return;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("mkdir {}: {e}", parent.display()));
    }
    std::fs::write(path, content).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn run_gen(dir: &Path) {
    match codegen::run(dir).expect("spec gen must not error") {
        codegen::Outcome::Generated(files) => {
            assert!(!files.is_empty(), "gen must report files");
        }
        other => panic!("expected Generated, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// bash
// ---------------------------------------------------------------------------

/// The real bash step implementations; `holds` controls whether scenario A's
/// assertion is true (the red/green flip).
fn bash_steps(holds: bool) -> String {
    let expected = if holds { 2 } else { 3 };
    format!(
        "step_a_seeded_counter() {{ counter=1; }}\n\
         step_the_counter_is_bumped() {{ counter=$((counter + 1)); }}\n\
         step_the_counter_holds_two() {{ [ \"$counter\" -eq {expected} ]; }}\n"
    )
}

#[test]
fn bash_round_trip_a_passes_b_undefined_then_a_fails() {
    if !tool_available("bats") {
        eprintln!("SKIP: bats not on PATH — install bats-core to run the bash round trip");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    write(
        &dir.join("craftsman.toml"),
        "[project]\nname = \"bash-round-trip\"\nstacks = [\"bash\"]\n",
    );
    write(&dir.join("SPEC.md"), ROUND_TRIP_SPEC);
    run_gen(dir);
    assert!(dir.join("tests/generated_spec.bats").is_file());
    assert!(dir.join("tests/steps.bash.template").is_file());

    // Implement scenario A only; B's step stays a template stub.
    write(&dir.join("tests/steps.bash"), &bash_steps(true));
    let green = verify::run(dir, &Selection::All).expect("verify runs");
    assert_trio_phase(&green, Status::Passed, Status::Undefined, "green phase");
    assert_eq!(
        green.outcome,
        Outcome::Failed,
        "undefined keeps the gate red"
    );

    // Break A: real failure, not undefined.
    write(&dir.join("tests/steps.bash"), &bash_steps(false));
    let red = verify::run(dir, &Selection::All).expect("verify runs");
    assert_trio_phase(&red, Status::Failed, Status::Undefined, "red phase");
}

#[test]
fn bash_scenario_filter_selects_exactly_one() {
    if !tool_available("bats") {
        eprintln!("SKIP: bats not on PATH — install bats-core to run the bash round trip");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    write(
        &dir.join("craftsman.toml"),
        "[project]\nname = \"bash-filter\"\nstacks = [\"bash\"]\n",
    );
    write(&dir.join("SPEC.md"), ROUND_TRIP_SPEC);
    run_gen(dir);
    write(&dir.join("tests/steps.bash"), &bash_steps(true));

    let one = verify::run(dir, &Selection::Scenario("Scenario A passes".to_owned()))
        .expect("verify runs");
    assert_eq!(
        statuses(&one),
        vec![("Scenario A passes".to_owned(), Status::Passed)]
    );
    assert_eq!(one.outcome, Outcome::Passed);

    // A name absent from the spec is exit-4 territory (empty selection),
    // resolved before bats ever runs.
    let none =
        verify::run(dir, &Selection::Scenario("No such scenario".to_owned())).expect("verify runs");
    assert_eq!(none.outcome, Outcome::EmptySelection);
}

// ---------------------------------------------------------------------------
// swift
// ---------------------------------------------------------------------------

const PACKAGE_SWIFT: &str = r#"// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Fixture",
    targets: [
        .target(name: "Fixture"),
        .testTarget(name: "FixtureTests", dependencies: ["Fixture"]),
    ]
)
"#;

/// The real swift step implementations; scenario B's step keeps its
/// generated stub body (the Undefined marker).
fn swift_steps(holds: bool) -> String {
    let expected = if holds { 2 } else { 3 };
    format!(
        "import Testing\n\n\
         struct SpecSteps {{\n\
         \x20   var counter = 0\n\n\
         \x20   mutating func step_a_seeded_counter() throws {{ counter = 1 }}\n\
         \x20   mutating func step_the_counter_is_bumped() throws {{ counter += 1 }}\n\
         \x20   mutating func step_the_counter_holds_two() throws {{\n\
         \x20       #expect(counter == {expected}, \"counter was \\(counter)\")\n\
         \x20   }}\n\n\
         \x20   mutating func step_an_unwritten_step() throws {{\n\
         \x20       #expect(Bool(false), \"step not implemented: Given an unwritten step\")\n\
         \x20   }}\n\
         }}\n"
    )
}

#[test]
fn swift_round_trip_a_passes_b_undefined_then_a_fails() {
    let Some(version) = swift_version() else {
        eprintln!("SKIP: swift not on PATH — install Xcode/Swift to run the swift round trip");
        return;
    };
    if version < (6, 2) {
        eprintln!(
            "SKIP: swift {}.{} < 6.2 — SE-0451 raw identifiers (generated test \
             names) need a newer toolchain",
            version.0, version.1
        );
        return;
    }

    // Stable path so the compiled .build/ is reused across runs (module docs).
    let dir = std::env::temp_dir().join("craftsman-swift-roundtrip-fixture");
    write(
        &dir.join("craftsman.toml"),
        "[project]\nname = \"swift-round-trip\"\nstacks = [\"swift\"]\n\n\
         [verify.swift]\nswift-tests-dir = \"Tests/FixtureTests\"\n",
    );
    write(&dir.join("SPEC.md"), ROUND_TRIP_SPEC);
    write(&dir.join("Package.swift"), PACKAGE_SWIFT);
    write(
        &dir.join("Sources/Fixture/Fixture.swift"),
        "public struct Fixture {}\n",
    );
    let _ = std::fs::remove_dir_all(dir.join(".craftsman"));
    run_gen(&dir);
    assert!(
        dir.join("Tests/FixtureTests/Generated/SpecScenarios.swift")
            .is_file()
    );
    assert!(
        dir.join("Tests/FixtureTests/Steps.swift.template")
            .is_file()
    );

    // Implement scenario A; keep B's generated stub verbatim.
    write(
        &dir.join("Tests/FixtureTests/Steps.swift"),
        &swift_steps(true),
    );
    let start = std::time::Instant::now();
    let green = verify::run(&dir, &Selection::All).expect("verify runs");
    eprintln!(
        "swift round trip: green phase in {:.1}s",
        start.elapsed().as_secs_f32()
    );
    assert_trio_phase(&green, Status::Passed, Status::Undefined, "green phase");
    assert_eq!(
        green.outcome,
        Outcome::Failed,
        "undefined keeps the gate red"
    );

    // Break A: real failure, not undefined.
    write(
        &dir.join("Tests/FixtureTests/Steps.swift"),
        &swift_steps(false),
    );
    let start = std::time::Instant::now();
    let red = verify::run(&dir, &Selection::All).expect("verify runs");
    eprintln!(
        "swift round trip: red phase in {:.1}s",
        start.elapsed().as_secs_f32()
    );
    assert_trio_phase(&red, Status::Failed, Status::Undefined, "red phase");
    let failure = red
        .results()
        .next()
        .and_then(|r| r.failure.as_deref())
        .expect("failure detail");
    assert!(failure.contains("counter was 2"), "{failure}");
}

// ---------------------------------------------------------------------------
// gen ownership rules (no runner needed — pure file behavior)
// ---------------------------------------------------------------------------

#[test]
fn gen_regenerates_ours_and_never_touches_theirs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    write(
        &dir.join("craftsman.toml"),
        "[project]\nname = \"ownership\"\nstacks = [\"bash\"]\n",
    );
    write(&dir.join("SPEC.md"), ROUND_TRIP_SPEC);
    run_gen(dir);

    let runner = dir.join("tests/generated_spec.bats");
    let template = dir.join("tests/steps.bash.template");
    let steps = dir.join("tests/steps.bash");

    // Hand-modify all three; the runner is ours (rewritten), the template
    // and step file are theirs (untouched).
    std::fs::write(&runner, "# vandalized\n").expect("write");
    std::fs::write(&template, "# my hand-tuned stubs\n").expect("write");
    std::fs::write(&steps, "# my real steps\n").expect("write");
    run_gen(dir);

    let runner_text = std::fs::read_to_string(&runner).expect("read");
    assert!(runner_text.contains("GENERATED"), "ours is regenerated");
    assert_eq!(
        std::fs::read_to_string(&template).expect("read"),
        "# my hand-tuned stubs\n",
        "the template is theirs once created"
    );
    assert_eq!(
        std::fs::read_to_string(&steps).expect("read"),
        "# my real steps\n",
        "real step files are never written by gen"
    );
}

// ---------------------------------------------------------------------------
// plumbing
// ---------------------------------------------------------------------------

fn tool_available(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// `(major, minor)` from `swift --version`, when swift is runnable.
fn swift_version() -> Option<(u32, u32)> {
    let out = Command::new("swift").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let rest = text.split("Swift version ").nth(1)?;
    let mut parts = rest.split(|c: char| !c.is_ascii_digit());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

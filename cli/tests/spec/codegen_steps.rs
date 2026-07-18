//! Step definitions — the recovered code-gen verify scenarios (Batch 11):
//! the swift assertion failure (`@requires-swift`) and the xcodebuild
//! pass/undefined/fail trio (`@requires-xcode`). Both fixtures live at
//! stable temp paths so compiled state survives across runs.

use std::path::PathBuf;

use cucumber::{given, then};

use crate::{CliWorld, fixtures};

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

const COUNTER_SPEC: &str = "Feature: Counter\n\n  Scenario: The counter round trip\n    Given a seeded counter\n    When the counter is bumped\n    Then the counter holds two\n";

/// Steps whose bump overshoots: the counter ends at 3, the assertion
/// expects 2 — a real failure naming the actual value, never undefined.
const OVERSHOOTING_STEPS: &str = "import Testing\n\nstruct SpecSteps {\n    var counter = 0\n\n    mutating func step_a_seeded_counter() throws { counter = 1 }\n    mutating func step_the_counter_is_bumped() throws { counter += 2 }\n    mutating func step_the_counter_holds_two() throws {\n        #expect(counter == 2, \"counter was \\(counter)\")\n    }\n}\n";

#[given(
    "a swift-stack craftsman project with generated scenarios whose step asserts a counter holds 2"
)]
fn swift_project_with_generated_scenarios(w: &mut CliWorld) {
    let dir = fixtures::stable_dir("craftsman-spec-swift-red-fixture");
    std::fs::create_dir_all(dir.join("Sources/Fixture")).expect("mkdirs");
    let write = |rel: &str, content: &str| {
        std::fs::write(dir.join(rel), content).unwrap_or_else(|e| panic!("write {rel}: {e}"));
    };
    write(
        "craftsman.toml",
        "[project]\nname = \"swift-red\"\nstacks = [\"swift\"]\n\n[verify.swift]\nswift-tests-dir = \"Tests/FixtureTests\"\n",
    );
    write("SPEC.md", COUNTER_SPEC);
    write("Package.swift", PACKAGE_SWIFT);
    write(
        "Sources/Fixture/Fixture.swift",
        "public struct Fixture {}\n",
    );
    fixtures::scrub(&dir, &[".craftsman"]);
    w.fixed_dir = Some(dir);
    w.prime(&["spec", "gen"]);
}

#[given("the step implementation makes the counter hold 3")]
fn step_makes_counter_hold_three(w: &mut CliWorld) {
    w.write("Tests/FixtureTests/Steps.swift", OVERSHOOTING_STEPS);
}

#[then("the scenario is reported failed, not undefined")]
fn reported_failed_not_undefined(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("1 failed") && combined.contains("0 undefined"),
        "the verdict must be a failure, not an undefined step:\n{combined}"
    );
}

#[then("the failure detail names the actual counter value")]
fn failure_names_counter_value(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("counter was 3"),
        "the assertion evidence must name the actual value:\n{combined}"
    );
}

#[given(
    "a swift-apple craftsman project with one passing, one unimplemented, and one failing scenario"
)]
fn xcode_project_with_trio(w: &mut CliWorld) {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/xcode-app");
    let dir = fixtures::stable_dir("craftsman-spec-xcode-fixture");
    fixtures::copy_tree(&fixture, &dir);
    fixtures::scrub(&dir, &[".craftsman"]);
    // The committed fixture is pass/pass/undefined; breaking the counter
    // assertion turns one pass into a genuine failure.
    let steps = dir.join("Tests/XcodeAppTests/Steps.swift");
    let text = std::fs::read_to_string(&steps).expect("read fixture steps");
    std::fs::write(&steps, text.replace("counter == 2", "counter == 3")).expect("write steps");
    w.fixed_dir = Some(dir);
}

#[then("the three scenarios are reported as passed, undefined, and failed respectively")]
fn trio_reported_distinctly(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("1 passed, 1 failed, 1 undefined"),
        "the trio must land in three distinct verdicts:\n{combined}"
    );
}

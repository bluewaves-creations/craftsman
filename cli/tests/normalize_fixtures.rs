//! One test per real runner fixture in `tests/fixtures/` — ported from
//! `spikes/s2-normalizer` (see ADR-002 for the commands that produced them),
//! plus the cucumber-rs `output-json` fixture backing ADR-003 and the Swift
//! Testing event-stream fixtures backing ADR-001 (`spikes/s1-swift-codegen`,
//! Batch 5).

use craftsman::verify::adapters::xcodebuild::parse_xcresult_tests;
use craftsman::verify::normalize::{
    CucumberJsonDialect, JunitDialect, ScenarioResult, Status, parse_cucumber_json, parse_junit,
    parse_messages_ndjson, parse_swift_events_jsonl,
};

fn fixture(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");
    std::fs::read_to_string(format!("{path}{name}")).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn triple(results: &[ScenarioResult]) -> Vec<(&str, Status)> {
    results
        .iter()
        .map(|r| (r.scenario.as_str(), r.status))
        .collect()
}

const PASSING: &str = "Add an item to the list";
const FAILING: &str = "Adding one item yields two items";
const UNDEFINED: &str = "Remove an item from the list";

#[test]
fn messages_ndjson_from_cucumber_js() {
    let r = parse_messages_ndjson(&fixture("msgs.ndjson")).expect("fixture must normalize");
    assert_eq!(
        triple(&r),
        vec![
            (PASSING, Status::Passed),
            (FAILING, Status::Failed),
            // cucumber-js reports the unimplemented step as UNDEFINED outright.
            (UNDEFINED, Status::Undefined),
        ]
    );
    assert!(r.iter().all(|x| x.feature == "Todo list"));
    assert!(
        r[1].failure
            .as_deref()
            .expect("failure message")
            .contains("1 !== 2")
    );
}

#[test]
fn cucumber_json_from_cucumber_js() {
    let r = parse_cucumber_json(&fixture("ts.json"), CucumberJsonDialect::Generic)
        .expect("fixture must normalize");
    assert_eq!(
        triple(&r),
        vec![
            (PASSING, Status::Passed),
            (FAILING, Status::Failed),
            // step statuses observed: passed / undefined / skipped → worst = UNDEFINED.
            (UNDEFINED, Status::Undefined),
        ]
    );
}

#[test]
fn cucumber_json_from_pytest_bdd() {
    let r = parse_cucumber_json(&fixture("py.json"), CucumberJsonDialect::Generic)
        .expect("fixture must normalize");
    assert_eq!(
        triple(&r),
        vec![
            (PASSING, Status::Passed),
            (FAILING, Status::Failed),
            // OBSERVED TRAP: pytest-bdd 8.1 drops the undefined step (and all
            // later steps) from cucumber-json, so the scenario looks PASSED.
            // The real signal lives only in the JUnit XML / exit code.
            (UNDEFINED, Status::Passed),
        ]
    );
}

#[test]
fn cucumber_json_from_cucumber_rs() {
    // ADR-003: the rust verify adapter ingests this format. Names are clean
    // (no `Scenario: …: path:line:col` mangling) and the unmatched step's
    // "skipped" status maps to UNDEFINED under the CucumberRs dialect.
    let r = parse_cucumber_json(&fixture("rust.json"), CucumberJsonDialect::CucumberRs)
        .expect("fixture must normalize");
    assert_eq!(
        triple(&r),
        vec![
            (PASSING, Status::Passed),
            (FAILING, Status::Failed),
            (UNDEFINED, Status::Undefined),
        ]
    );
    assert!(r.iter().all(|x| x.feature == "Todo list"));
    assert!(
        r[1].failure
            .as_deref()
            .expect("failure message")
            .contains("Step panicked")
    );
}

#[test]
fn junit_from_pytest_bdd() {
    let r = parse_junit(&fixture("py-junit.xml"), JunitDialect::PytestBdd)
        .expect("fixture must normalize");
    assert_eq!(
        triple(&r),
        vec![
            // pytest mangles scenario names into test ids — round-tripping the
            // real name requires the cucumber-json (or a name-mangling map).
            ("test_add_an_item_to_the_list", Status::Passed),
            ("test_adding_one_item_yields_two_items", Status::Failed),
            // failure message contains StepDefinitionNotFoundError → UNDEFINED.
            ("test_remove_an_item_from_the_list", Status::Undefined),
        ]
    );
    assert!(
        r[2].failure
            .as_deref()
            .expect("failure message")
            .contains("StepDefinitionNotFoundError")
    );
}

#[test]
fn junit_from_cucumber_rs() {
    let r = parse_junit(&fixture("rust-junit.xml"), JunitDialect::CucumberRs)
        .expect("fixture must normalize");
    assert_eq!(
        triple(&r),
        vec![
            (PASSING, Status::Passed),
            // cucumber-rs reports assert failures as `Step Panicked`.
            (FAILING, Status::Failed),
            // cucumber-rs emits <skipped/> for the unmatched step; only the
            // `?  <step>` marker in system-out distinguishes it → UNDEFINED.
            (UNDEFINED, Status::Undefined),
        ]
    );
    assert!(r.iter().all(|x| x.feature == "Todo list"));
    assert!(
        r[1].failure
            .as_deref()
            .expect("failure message")
            .contains("Step panicked")
    );
}

#[test]
fn junit_from_bats() {
    let r = parse_junit(&fixture("bash-junit.xml"), JunitDialect::Bats)
        .expect("fixture must normalize");
    assert_eq!(
        triple(&r),
        vec![
            (PASSING, Status::Passed),
            (FAILING, Status::Failed),
            // bats has no step concept; craftsman's generated `skip "step not
            // implemented: ..."` convention is recovered as UNDEFINED.
            (UNDEFINED, Status::Undefined),
        ]
    );
    assert_eq!(r[0].feature, "todo.bats");
}

#[test]
fn swift_events_jsonl_all_green() {
    // spikes/s1-swift-codegen/ev0.jsonl — the S1 spike run, all passing,
    // including the parameterized outline (3 Examples rows, one function).
    let r = parse_swift_events_jsonl(&fixture("swift-ev0.jsonl")).expect("fixture must normalize");
    assert_eq!(
        triple(&r),
        vec![
            ("Adding a todo shows it in the list", Status::Passed),
            ("Completing a todo moves it to done", Status::Passed),
            (
                "Rejecting an invalid quantity keeps the cart unchanged",
                Status::Passed
            ),
        ]
    );
    assert!(r.iter().all(|x| x.feature == "Todo management"), "{r:?}");
    assert!(r.iter().all(|x| x.failure.is_none()));
    assert!(r.iter().all(|x| x.duration_ms.is_some()));
}

#[test]
fn swift_events_jsonl_with_real_failure() {
    // spikes/s1-swift-codegen/evfail.jsonl — deliberate assertion failure.
    let r =
        parse_swift_events_jsonl(&fixture("swift-evfail.jsonl")).expect("fixture must normalize");
    assert_eq!(
        triple(&r),
        vec![
            ("Adding a todo shows it in the list", Status::Failed),
            ("Completing a todo moves it to done", Status::Passed),
            (
                "Rejecting an invalid quantity keeps the cart unchanged",
                Status::Passed
            ),
        ]
    );
    assert!(
        r[0].failure
            .as_deref()
            .expect("failure message")
            .contains("expected list to contain Buy oat milk")
    );
}

#[test]
fn swift_events_jsonl_stub_markers_are_undefined() {
    // A real run of craftsman-generated code with every step left as its
    // template stub: all issues carry the "step not implemented:" marker →
    // UNDEFINED, not Failed (message-prefix dialect). The parameterized
    // scenario's failure detail names the failing rows.
    let r = parse_swift_events_jsonl(&fixture("swift-ev-undefined.jsonl"))
        .expect("fixture must normalize");
    assert_eq!(
        triple(&r),
        vec![
            ("Adding a todo shows it in the list", Status::Undefined),
            (
                "Rejecting an invalid quantity keeps the cart unchanged",
                Status::Undefined
            ),
        ]
    );
    let detail = r[1].failure.as_deref().expect("failure detail");
    assert!(detail.contains("step not implemented: "), "{detail}");
    assert!(detail.contains(r#"[0, "zero"]"#), "{detail}");
}

#[test]
fn swift_xunit_sibling_is_a_coarse_fallback() {
    // spikes/s1-swift-codegen/rfail-swift-testing.xml — the xunit sibling of
    // the failing run. Raw-identifier names are recovered; the feature stays
    // the mangled Target.Suite classname (coarse by design, ADR-001).
    let r = parse_junit(&fixture("swift-xunit-fail.xml"), JunitDialect::SwiftTesting)
        .expect("fixture must normalize");
    assert_eq!(
        triple(&r),
        vec![
            ("Completing a todo moves it to done", Status::Passed),
            ("Adding a todo shows it in the list", Status::Failed),
            (
                "Rejecting an invalid quantity keeps the cart unchanged",
                Status::Passed
            ),
        ]
    );
    assert!(
        r.iter()
            .all(|x| x.feature == "SpecSpikeTests.TodoManagementFeature")
    );
}

#[test]
fn xcresult_tests_json_fixture_normalizes() {
    // tests/fixtures/xcresult-tests.json — captured from a REAL bundle
    // (Batch 9a probe, Xcode 26.6): `xcodebuild test` on the S1-style
    // SwiftPM package scheme with seeded pass/fail/undefined/param-row
    // outcomes, then `xcrun xcresulttool get test-results tests --path …`;
    // device fields scrubbed, structure untouched.
    let r = parse_xcresult_tests(&fixture("xcresult-tests.json")).expect("fixture must normalize");
    assert_eq!(r.len(), 10, "{r:?}");

    let by_name = |name: &str| {
        r.iter()
            .find(|x| x.scenario == name)
            .unwrap_or_else(|| panic!("{name} missing"))
    };
    let passing = by_name("Adding a todo shows it in the list");
    assert_eq!(passing.status, Status::Passed);
    assert_eq!(
        passing.feature, "Todo management",
        "Feature: prefix stripped"
    );
    assert!(passing.duration_ms.is_some());
    assert!(passing.failure.is_none());

    let failing = by_name("A genuinely failing scenario");
    assert_eq!(failing.status, Status::Failed);
    assert_eq!(failing.feature, "Seeded outcomes");
    assert!(
        failing
            .failure
            .as_deref()
            .expect("failure detail")
            .contains("expected the impossible")
    );

    // The stub marker maps to Undefined — same dialect as the JSONL path.
    let undefined = by_name("An undefined scenario");
    assert_eq!(undefined.status, Status::Undefined);

    // Parameterized: one result for the function; the failing row's
    // arguments label its failure detail.
    let outline = by_name("A row failing outline");
    assert_eq!(outline.status, Status::Failed);
    let detail = outline.failure.as_deref().expect("failure detail");
    assert!(detail.contains(r#"[7, "boom"]"#), "{detail}");
    assert!(detail.contains("quantity too big"), "{detail}");
}

#[test]
fn unknown_status_is_rejected_loudly() {
    let doc = r#"[{"name":"F","elements":[{"type":"scenario","name":"S",
        "steps":[{"name":"s","result":{"status":"exploded"}}]}]}]"#;
    let err = parse_cucumber_json(doc, CucumberJsonDialect::Generic)
        .expect_err("unknown status must be an error, never a default");
    assert!(err.to_string().contains("exploded"), "{err}");
}

#[test]
fn malformed_input_is_rejected_loudly() {
    assert!(parse_cucumber_json("not json", CucumberJsonDialect::Generic).is_err());
    assert!(parse_messages_ndjson("{\"pickle\":{}}\nnot json\n").is_err());
    assert!(parse_junit("<not-xml", JunitDialect::Bats).is_err());
}

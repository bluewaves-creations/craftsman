//! One test per real runner fixture in `tests/fixtures/` — ported from
//! `spikes/s2-normalizer` (see ADR-002 for the commands that produced them),
//! plus the cucumber-rs `output-json` fixture backing ADR-003.

use craftsman::verify::normalize::{
    CucumberJsonDialect, JunitDialect, ScenarioResult, Status, parse_cucumber_json, parse_junit,
    parse_messages_ndjson,
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

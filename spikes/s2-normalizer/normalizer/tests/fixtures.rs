//! One test per real fixture in ../fixtures/ (produced by the sample projects
//! in ../samples/ — see ADR-002 for the commands).

use normalizer::{JunitDialect, ScenarioResult, Status, parse_cucumber_json, parse_junit, parse_messages_ndjson};

fn fixture(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/");
    std::fs::read_to_string(format!("{path}{name}")).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn triple(results: &[ScenarioResult]) -> Vec<(&str, Status)> {
    results.iter().map(|r| (r.scenario.as_str(), r.status)).collect()
}

const PASSING: &str = "Add an item to the list";
const FAILING: &str = "Adding one item yields two items";
const UNDEFINED: &str = "Remove an item from the list";

#[test]
fn messages_ndjson_from_cucumber_js() {
    let r = parse_messages_ndjson(&fixture("msgs.ndjson"));
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
    assert!(r[1].failure.as_deref().unwrap().contains("1 !== 2"));
}

#[test]
fn cucumber_json_from_cucumber_js() {
    let r = parse_cucumber_json(&fixture("ts.json"));
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
    let r = parse_cucumber_json(&fixture("py.json"));
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
fn junit_from_pytest_bdd() {
    let r = parse_junit(&fixture("py-junit.xml"), JunitDialect::PytestBdd);
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
    assert!(r[2].failure.as_deref().unwrap().contains("StepDefinitionNotFoundError"));
}

#[test]
fn junit_from_cucumber_rs() {
    let r = parse_junit(&fixture("rust-junit.xml"), JunitDialect::CucumberRs);
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
    assert!(r[1].failure.as_deref().unwrap().contains("Step panicked"));
}

#[test]
fn junit_from_bats() {
    let r = parse_junit(&fixture("bash-junit.xml"), JunitDialect::Bats);
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

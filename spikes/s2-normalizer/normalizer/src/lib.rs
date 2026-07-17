//! Spike S2 — six-status result normalizer over real runner fixtures.
//!
//! Three parsers (Cucumber Messages NDJSON, cucumber-json, JUnit XML), each
//! producing `Vec<ScenarioResult>`. JUnit needs a per-runner dialect because
//! plain JUnit cannot express UNDEFINED/PENDING — each runner smuggles that
//! information somewhere different (or nowhere, see the pytest-bdd trap).

use serde_json::Value;

/// Cucumber Messages status vocabulary — the schema v1 status set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Passed,
    Skipped,
    Pending,
    Undefined,
    Ambiguous,
    Failed,
}

/// Schema v1: one normalized row per executed scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioResult {
    pub feature: String,
    pub scenario: String,
    pub status: Status,
    pub duration_ms: Option<u64>,
    pub failure: Option<String>,
}

impl Status {
    /// Parse a cucumber status string (either JSON lowercase or Messages uppercase).
    fn from_cucumber(s: &str) -> Option<Status> {
        match s.to_ascii_lowercase().as_str() {
            "passed" => Some(Status::Passed),
            "failed" => Some(Status::Failed),
            "skipped" => Some(Status::Skipped),
            "pending" => Some(Status::Pending),
            "undefined" => Some(Status::Undefined),
            "ambiguous" => Some(Status::Ambiguous),
            _ => None,
        }
    }
}

/// Scenario status = most severe step status. `Ord` derives severity from
/// variant order: Passed < Skipped < Pending < Undefined < Ambiguous < Failed.
fn worst(statuses: impl IntoIterator<Item = Status>) -> Status {
    statuses.into_iter().max().unwrap_or(Status::Passed)
}

// ---------------------------------------------------------------------------
// Parser 1: Cucumber Messages NDJSON (@cucumber/cucumber --format message)
// ---------------------------------------------------------------------------

/// Joins gherkinDocument → pickle → testCase → testCaseStarted → testStepFinished.
pub fn parse_messages_ndjson(input: &str) -> Vec<ScenarioResult> {
    use std::collections::HashMap;

    let msgs: Vec<Value> = input
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    let mut feature = String::new();
    let mut pickle_names: HashMap<String, String> = HashMap::new(); // pickleId -> scenario name
    let mut testcase_pickle: HashMap<String, String> = HashMap::new(); // testCaseId -> pickleId
    let mut started_testcase: Vec<(String, String)> = Vec::new(); // (testCaseStartedId, testCaseId) in start order
    // testCaseStartedId -> (statuses, total nanos, first failure message)
    let mut runs: HashMap<String, (Vec<Status>, u64, Option<String>)> = HashMap::new();

    for m in &msgs {
        if let Some(doc) = m.get("gherkinDocument") {
            if let Some(name) = doc.pointer("/feature/name").and_then(Value::as_str) {
                feature = name.to_string();
            }
        } else if let Some(p) = m.get("pickle") {
            pickle_names.insert(
                p["id"].as_str().unwrap_or_default().to_string(),
                p["name"].as_str().unwrap_or_default().to_string(),
            );
        } else if let Some(tc) = m.get("testCase") {
            testcase_pickle.insert(
                tc["id"].as_str().unwrap_or_default().to_string(),
                tc["pickleId"].as_str().unwrap_or_default().to_string(),
            );
        } else if let Some(tcs) = m.get("testCaseStarted") {
            started_testcase.push((
                tcs["id"].as_str().unwrap_or_default().to_string(),
                tcs["testCaseId"].as_str().unwrap_or_default().to_string(),
            ));
        } else if let Some(tsf) = m.get("testStepFinished") {
            let run_id = tsf["testCaseStartedId"].as_str().unwrap_or_default().to_string();
            let result = &tsf["testStepResult"];
            let entry = runs.entry(run_id).or_default();
            if let Some(s) = result["status"].as_str().and_then(Status::from_cucumber) {
                entry.0.push(s);
            }
            entry.1 += result.pointer("/duration/seconds").and_then(Value::as_u64).unwrap_or(0)
                * 1_000_000_000
                + result.pointer("/duration/nanos").and_then(Value::as_u64).unwrap_or(0);
            if entry.2.is_none() {
                entry.2 = result["message"].as_str().map(str::to_string);
            }
        }
    }

    started_testcase
        .iter()
        .map(|(run_id, testcase_id)| {
            let scenario = testcase_pickle
                .get(testcase_id)
                .and_then(|pid| pickle_names.get(pid))
                .cloned()
                .unwrap_or_default();
            let (statuses, nanos, failure) = runs.remove(run_id).unwrap_or_default();
            ScenarioResult {
                feature: feature.clone(),
                scenario,
                status: worst(statuses),
                duration_ms: Some(nanos / 1_000_000),
                failure,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Parser 2: cucumber-json (pytest-bdd --cucumberjson, cucumber-js --format json)
// ---------------------------------------------------------------------------

/// Step-level statuses, worst-of aggregation. Durations are nanoseconds in
/// both pytest-bdd 8.1 and @cucumber/cucumber 13 output.
///
/// TRAP (observed): pytest-bdd omits the undefined step and everything after
/// it from `steps`, so an unimplemented scenario aggregates to PASSED here.
/// A pytest-bdd adapter must not trust cucumber-json alone for UNDEFINED.
pub fn parse_cucumber_json(input: &str) -> Vec<ScenarioResult> {
    let doc: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    let mut out = Vec::new();
    for feat in doc.as_array().into_iter().flatten() {
        let feature = feat["name"].as_str().unwrap_or_default().to_string();
        for el in feat["elements"].as_array().into_iter().flatten() {
            if el["type"].as_str() == Some("background") {
                continue;
            }
            let steps = el["steps"].as_array().cloned().unwrap_or_default();
            let statuses: Vec<Status> = steps
                .iter()
                .filter_map(|s| s.pointer("/result/status")?.as_str().and_then(Status::from_cucumber))
                .collect();
            let nanos: u64 = steps
                .iter()
                .filter_map(|s| s.pointer("/result/duration")?.as_u64())
                .sum();
            let failure = steps.iter().find_map(|s| {
                s.pointer("/result/error_message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
            out.push(ScenarioResult {
                feature: feature.clone(),
                scenario: el["name"].as_str().unwrap_or_default().to_string(),
                status: worst(statuses),
                duration_ms: Some(nanos / 1_000_000),
                failure,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Parser 3: JUnit XML (pytest-bdd --junitxml, cucumber-rs output-junit, bats)
// ---------------------------------------------------------------------------

/// JUnit itself only knows pass/failure/error/skipped. Each runner needs a
/// dialect to recover the six-status vocabulary from runner-specific markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JunitDialect {
    /// testcase name = mangled pytest id; UNDEFINED appears as a failure whose
    /// message contains `StepDefinitionNotFoundError`.
    PytestBdd,
    /// testcase name = `Scenario: <name>: <path>:<line>:<col>`; UNDEFINED
    /// appears as `<skipped/>` with a `?  <step>` marker in system-out.
    CucumberRs,
    /// testcase name = scenario name verbatim (craftsman generates the .bats);
    /// UNDEFINED appears as a skip whose reason starts `step not implemented`.
    Bats,
}

pub fn parse_junit(input: &str, dialect: JunitDialect) -> Vec<ScenarioResult> {
    let doc = match roxmltree::Document::parse(input) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for suite in doc.descendants().filter(|n| n.has_tag_name("testsuite")) {
        let suite_name = suite.attribute("name").unwrap_or_default();
        for case in suite.children().filter(|n| n.has_tag_name("testcase")) {
            let raw_name = case.attribute("name").unwrap_or_default();
            let classname = case.attribute("classname").unwrap_or_default();
            let duration_ms = case
                .attribute("time")
                .and_then(|t| t.parse::<f64>().ok())
                .map(|secs| (secs * 1000.0) as u64);

            let child = |tag: &str| case.children().find(|n| n.has_tag_name(tag));
            let text_of = |n: roxmltree::Node| {
                let attr_msg = n.attribute("message").unwrap_or_default();
                let body = n.text().unwrap_or_default();
                format!("{attr_msg}{}{body}", if attr_msg.is_empty() { "" } else { "\n" })
                    .trim()
                    .to_string()
            };
            let system_out = child("system-out").map(text_of).unwrap_or_default();

            let (mut status, mut failure) = if let Some(f) = child("failure").or_else(|| child("error")) {
                (Status::Failed, Some(text_of(f)))
            } else if let Some(s) = child("skipped") {
                (Status::Skipped, Some(text_of(s)).filter(|t| !t.is_empty()))
            } else {
                (Status::Passed, None)
            };

            // Dialect quirk recovery, all empirically observed (see fixtures).
            let (feature, scenario) = match dialect {
                JunitDialect::PytestBdd => {
                    if status == Status::Failed
                        && failure.as_deref().is_some_and(|f| f.contains("StepDefinitionNotFoundError"))
                    {
                        status = Status::Undefined;
                    }
                    (classname.to_string(), raw_name.to_string())
                }
                JunitDialect::CucumberRs => {
                    if status == Status::Skipped && system_out.contains("?  ") {
                        status = Status::Undefined;
                    }
                    (
                        strip_cucumber_rs_name(suite_name, "Feature: "),
                        strip_cucumber_rs_name(raw_name, "Scenario: "),
                    )
                }
                JunitDialect::Bats => {
                    if status == Status::Skipped
                        && failure.as_deref().is_some_and(|f| f.starts_with("step not implemented"))
                    {
                        status = Status::Undefined;
                    }
                    (suite_name.to_string(), raw_name.to_string())
                }
            };
            if status == Status::Passed {
                failure = None;
            }
            out.push(ScenarioResult { feature, scenario, status, duration_ms, failure });
        }
    }
    out
}

/// cucumber-rs writes `Feature: <name>: <path>` / `Scenario: <name>: <path>:<line>:<col>`.
/// Strip the keyword prefix and the trailing `: <path...>` segment.
fn strip_cucumber_rs_name(raw: &str, prefix: &str) -> String {
    let rest = raw.strip_prefix(prefix).unwrap_or(raw);
    match rest.rfind(": ") {
        Some(idx) => rest[..idx].to_string(),
        None => rest.to_string(),
    }
}

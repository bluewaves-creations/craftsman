//! Six-status result normalizer — schema v1, frozen by ADR-002.
//!
//! Three parsers (Cucumber Messages NDJSON, cucumber-json, `JUnit` XML), each
//! producing `Vec<ScenarioResult>`. `JUnit` needs a per-runner dialect because
//! plain `JUnit` cannot express UNDEFINED/PENDING — each runner smuggles that
//! information somewhere different (or nowhere: the pytest-bdd trap, where
//! cucumber-json silently omits undefined steps).
//!
//! Ported from `spikes/s2-normalizer` (fixture-proven skeletons kept
//! verbatim); error handling redone per repo conventions — malformed input
//! and unknown status strings are rejected loudly instead of being swallowed.

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

/// Cucumber Messages status vocabulary — the schema v1 status set.
///
/// Variant order doubles as severity: scenario status = `max` over step
/// statuses (Passed < Skipped < Pending < Undefined < Ambiguous < Failed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Passed,
    Skipped,
    Pending,
    Undefined,
    Ambiguous,
    Failed,
}

/// Schema v1 (ADR-002, frozen): one normalized row per executed scenario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScenarioResult {
    pub feature: String,
    pub scenario: String,
    pub status: Status,
    pub duration_ms: Option<u64>,
    pub failure: Option<String>,
}

/// Errors normalizing runner output. Exit code 3 territory: a result file the
/// CLI cannot read truthfully is an orchestrator failure, never a pass.
#[derive(Debug, Error)]
pub enum NormalizeError {
    #[error("malformed JSON in {context}")]
    Json {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("{context}: expected {expected}")]
    UnexpectedShape {
        context: &'static str,
        expected: &'static str,
    },
    #[error("missing field {field:?} in {context}")]
    MissingField {
        field: &'static str,
        context: &'static str,
    },
    #[error(
        "unknown result status {status:?} in {context} — schema v1 knows passed/failed/skipped/pending/undefined/ambiguous"
    )]
    UnknownStatus {
        status: String,
        context: &'static str,
    },
    #[error("malformed JUnit XML")]
    Xml(#[from] roxmltree::Error),
}

impl Status {
    /// Parse a cucumber status string (JSON lowercase or Messages uppercase).
    ///
    /// # Errors
    /// [`NormalizeError::UnknownStatus`] on anything outside the six-status
    /// vocabulary — loud rejection, never a silent default.
    pub fn from_cucumber(s: &str, context: &'static str) -> Result<Self, NormalizeError> {
        match s.to_ascii_lowercase().as_str() {
            "passed" => Ok(Self::Passed),
            "failed" => Ok(Self::Failed),
            "skipped" => Ok(Self::Skipped),
            "pending" => Ok(Self::Pending),
            "undefined" => Ok(Self::Undefined),
            "ambiguous" => Ok(Self::Ambiguous),
            _ => Err(NormalizeError::UnknownStatus {
                status: s.to_owned(),
                context,
            }),
        }
    }
}

/// Scenario status = most severe step status. `Ord` derives severity from
/// variant order.
fn worst(statuses: impl IntoIterator<Item = Status>) -> Status {
    statuses.into_iter().max().unwrap_or(Status::Passed)
}

fn str_field(
    v: &Value,
    field: &'static str,
    context: &'static str,
) -> Result<String, NormalizeError> {
    v.get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(NormalizeError::MissingField { field, context })
}

// ---------------------------------------------------------------------------
// Parser 1: Cucumber Messages NDJSON (@cucumber/cucumber --format message)
// ---------------------------------------------------------------------------

/// Joins gherkinDocument → pickle → testCase → testCaseStarted →
/// testStepFinished (the fixture-proven chain from ADR-002).
///
/// # Errors
/// Any non-JSON line, missing join id, or unknown status is rejected.
pub fn parse_messages_ndjson(input: &str) -> Result<Vec<ScenarioResult>, NormalizeError> {
    use std::collections::HashMap;

    const CTX: &str = "Cucumber Messages NDJSON";

    let mut feature = String::new();
    // pickleId -> scenario name
    let mut pickle_names: HashMap<String, String> = HashMap::new();
    // testCaseId -> pickleId
    let mut testcase_pickle: HashMap<String, String> = HashMap::new();
    // (testCaseStartedId, testCaseId) in start order
    let mut started_testcase: Vec<(String, String)> = Vec::new();
    // testCaseStartedId -> (statuses, total nanos, first failure message)
    let mut runs: HashMap<String, (Vec<Status>, u64, Option<String>)> = HashMap::new();

    for line in input.lines().filter(|l| !l.trim().is_empty()) {
        let m: Value = serde_json::from_str(line).map_err(|source| NormalizeError::Json {
            context: CTX,
            source,
        })?;
        if let Some(doc) = m.get("gherkinDocument") {
            if let Some(name) = doc.pointer("/feature/name").and_then(Value::as_str) {
                name.clone_into(&mut feature);
            }
        } else if let Some(p) = m.get("pickle") {
            pickle_names.insert(str_field(p, "id", CTX)?, str_field(p, "name", CTX)?);
        } else if let Some(tc) = m.get("testCase") {
            testcase_pickle.insert(str_field(tc, "id", CTX)?, str_field(tc, "pickleId", CTX)?);
        } else if let Some(tcs) = m.get("testCaseStarted") {
            started_testcase.push((
                str_field(tcs, "id", CTX)?,
                str_field(tcs, "testCaseId", CTX)?,
            ));
        } else if let Some(tsf) = m.get("testStepFinished") {
            let run_id = str_field(tsf, "testCaseStartedId", CTX)?;
            let result = tsf
                .get("testStepResult")
                .ok_or(NormalizeError::MissingField {
                    field: "testStepResult",
                    context: CTX,
                })?;
            let status = Status::from_cucumber(&str_field(result, "status", CTX)?, CTX)?;
            let entry = runs.entry(run_id).or_default();
            entry.0.push(status);
            entry.1 += result
                .pointer("/duration/seconds")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                * 1_000_000_000
                + result
                    .pointer("/duration/nanos")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
            if entry.2.is_none() {
                entry.2 = result
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
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
                .ok_or(NormalizeError::MissingField {
                    field: "pickle",
                    context: CTX,
                })?;
            let (statuses, nanos, failure) = runs.remove(run_id).unwrap_or_default();
            Ok(ScenarioResult {
                feature: feature.clone(),
                scenario,
                status: worst(statuses),
                duration_ms: Some(nanos / 1_000_000),
                failure,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Parser 2: cucumber-json
// (pytest-bdd --cucumberjson, cucumber-js --format json, cucumber-rs output-json)
// ---------------------------------------------------------------------------

/// Runner-specific reading of cucumber-json step statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CucumberJsonDialect {
    /// Statuses taken at face value (pytest-bdd, @cucumber/cucumber).
    ///
    /// TRAP (observed, ADR-002): pytest-bdd omits the undefined step and
    /// everything after it from `steps`, so an unimplemented scenario
    /// aggregates to PASSED here. A pytest-bdd adapter must not trust
    /// cucumber-json alone for UNDEFINED.
    Generic,
    /// cucumber-rs 0.23 `output-json` (ADR-003): a `"skipped"` step means
    /// "no matching step definition" — cucumber-rs has no other source of
    /// step-level skips, and later steps are omitted — so it maps to
    /// UNDEFINED.
    CucumberRs,
}

/// Step-level statuses, worst-of aggregation. Durations are nanoseconds in
/// pytest-bdd 8.1, @cucumber/cucumber 13, and cucumber-rs 0.23 output.
///
/// # Errors
/// Malformed JSON, a non-array document, or an unknown step status.
pub fn parse_cucumber_json(
    input: &str,
    dialect: CucumberJsonDialect,
) -> Result<Vec<ScenarioResult>, NormalizeError> {
    const CTX: &str = "cucumber-json";

    let doc: Value = serde_json::from_str(input).map_err(|source| NormalizeError::Json {
        context: CTX,
        source,
    })?;
    let features = doc.as_array().ok_or(NormalizeError::UnexpectedShape {
        context: CTX,
        expected: "a top-level array of features",
    })?;

    let mut out = Vec::new();
    for feat in features {
        let feature = str_field(feat, "name", CTX)?;
        for el in feat
            .get("elements")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if el.get("type").and_then(Value::as_str) == Some("background") {
                continue;
            }
            let steps = el
                .get("steps")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut statuses = Vec::with_capacity(steps.len());
            for step in &steps {
                let result = step.get("result").ok_or(NormalizeError::MissingField {
                    field: "result",
                    context: CTX,
                })?;
                let mut status = Status::from_cucumber(&str_field(result, "status", CTX)?, CTX)?;
                if dialect == CucumberJsonDialect::CucumberRs && status == Status::Skipped {
                    status = Status::Undefined;
                }
                statuses.push(status);
            }
            let nanos: u64 = steps
                .iter()
                .filter_map(|s| s.pointer("/result/duration")?.as_u64())
                .sum();
            let failure = steps.iter().find_map(|s| {
                s.pointer("/result/error_message")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
            out.push(ScenarioResult {
                feature: feature.clone(),
                scenario: str_field(el, "name", CTX)?,
                status: worst(statuses),
                duration_ms: Some(nanos / 1_000_000),
                failure,
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Parser 3: JUnit XML (pytest-bdd --junitxml, cucumber-rs output-junit, bats)
// ---------------------------------------------------------------------------

/// `JUnit` itself only knows pass/failure/error/skipped. Each runner needs a
/// dialect to recover the six-status vocabulary from runner-specific markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JunitDialect {
    /// testcase name = mangled pytest id; UNDEFINED appears as a failure whose
    /// message contains `StepDefinitionNotFoundError`.
    PytestBdd,
    /// testcase name = `Scenario: <name>: <path>:<line>:<col>`; UNDEFINED
    /// appears as `<skipped/>` with a `?  <step>` marker in system-out.
    /// Kept as a fallback — the verify adapter ingests cucumber-json instead
    /// (ADR-003), where names are clean.
    CucumberRs,
    /// testcase name = scenario name verbatim (craftsman generates the .bats);
    /// UNDEFINED appears as a skip whose reason starts `step not implemented`.
    Bats,
}

/// # Errors
/// Malformed XML, or a `testcase` without a `name` attribute.
pub fn parse_junit(
    input: &str,
    dialect: JunitDialect,
) -> Result<Vec<ScenarioResult>, NormalizeError> {
    const CTX: &str = "JUnit XML";

    let doc = roxmltree::Document::parse(input)?;
    let mut out = Vec::new();
    for suite in doc.descendants().filter(|n| n.has_tag_name("testsuite")) {
        let suite_name = suite.attribute("name").unwrap_or_default();
        for case in suite.children().filter(|n| n.has_tag_name("testcase")) {
            let raw_name = case.attribute("name").ok_or(NormalizeError::MissingField {
                field: "name",
                context: CTX,
            })?;
            let classname = case.attribute("classname").unwrap_or_default();
            let duration_ms = case
                .attribute("time")
                .and_then(|t| t.parse::<f64>().ok())
                .map(seconds_to_ms);

            let child = |tag: &str| case.children().find(|n| n.has_tag_name(tag));
            let text_of = |n: roxmltree::Node<'_, '_>| {
                let attr_msg = n.attribute("message").unwrap_or_default();
                let body = n.text().unwrap_or_default();
                format!(
                    "{attr_msg}{}{body}",
                    if attr_msg.is_empty() { "" } else { "\n" }
                )
                .trim()
                .to_owned()
            };
            let system_out = child("system-out").map(text_of).unwrap_or_default();

            let (mut status, mut failure) =
                child("failure").or_else(|| child("error")).map_or_else(
                    || {
                        child("skipped").map_or((Status::Passed, None), |s| {
                            (Status::Skipped, Some(text_of(s)).filter(|t| !t.is_empty()))
                        })
                    },
                    |f| (Status::Failed, Some(text_of(f))),
                );

            // Dialect quirk recovery, all empirically observed (ADR-002).
            let (feature, scenario) = match dialect {
                JunitDialect::PytestBdd => {
                    if status == Status::Failed
                        && failure
                            .as_deref()
                            .is_some_and(|f| f.contains("StepDefinitionNotFoundError"))
                    {
                        status = Status::Undefined;
                    }
                    (classname.to_owned(), raw_name.to_owned())
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
                        && failure
                            .as_deref()
                            .is_some_and(|f| f.starts_with("step not implemented"))
                    {
                        status = Status::Undefined;
                    }
                    (suite_name.to_owned(), raw_name.to_owned())
                }
            };
            if status == Status::Passed {
                failure = None;
            }
            out.push(ScenarioResult {
                feature,
                scenario,
                status,
                duration_ms,
                failure,
            });
        }
    }
    Ok(out)
}

/// `JUnit` `time` is seconds as a float; results carry whole milliseconds.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "durations are small non-negative reals; clamped before the cast"
)]
fn seconds_to_ms(secs: f64) -> u64 {
    (secs.max(0.0) * 1000.0) as u64
}

/// cucumber-rs `JUnit` writes `Feature: <name>: <path>` /
/// `Scenario: <name>: <path>:<line>:<col>`. Strip the keyword prefix and the
/// trailing `: <path...>` segment. Known hazard (ADR-002): breaks if a name
/// itself contains `": "` — which is why verify ingests cucumber-json instead.
fn strip_cucumber_rs_name(raw: &str, prefix: &str) -> String {
    let rest = raw.strip_prefix(prefix).unwrap_or(raw);
    rest.rfind(": ")
        .map_or_else(|| rest.to_owned(), |idx| rest[..idx].to_owned())
}

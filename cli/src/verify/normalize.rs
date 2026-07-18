//! Six-status result normalizer — schema v1, frozen by ADR-002.
//!
//! Four parsers (Cucumber Messages NDJSON, cucumber-json, Swift Testing
//! event-stream JSONL, `JUnit` XML), each
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

/// The message-prefix dialect for craftsman-generated step stubs.
///
/// A failure or skip whose message starts with this marker means "no real
/// step implementation exists yet" and maps to [`Status::Undefined`], not
/// Failed/Skipped. The generators in `crate::codegen` (swift and bash)
/// write exactly this prefix into every stub.
pub const NOT_IMPLEMENTED_PREFIX: &str = "step not implemented: ";

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
// Parser 3: Swift Testing event stream JSONL v0 (ADR-001)
// (`swift test --experimental-event-stream-output … --experimental-event-stream-version 0`)
// ---------------------------------------------------------------------------

/// Parse the Swift Testing JSONL v0 event stream — the primary verdict
/// source for the swift stack (per-Examples-row results exist only here,
/// never in the xunit sibling; ADR-001).
///
/// Record shapes (all fixture-proven in `spikes/s1-swift-codegen/`):
/// - `kind:"test"`, `payload.kind:"function"` — discovery; `id` is the
///   3-part `Target.Suite/`name`()/File.swift:line:col` test ID and
///   `displayName` the raw scenario name.
/// - `kind:"test"`, `payload.kind:"suite"` — `displayName` is the
///   generated `Feature: <name>` suite title.
/// - `kind:"event"`, `payload.kind:"testStarted"/"testEnded"` — verdict:
///   `messages[].symbol` is `pass`/`fail`, keyed by `testID`; the
///   started→ended `instant.absolute` delta is the duration.
/// - `kind:"event"`, `payload.kind:"issueRecorded"` — failure detail;
///   parameterized rows carry `_testCase.displayName` (the argument
///   values).
///
/// Undefined detection (message-prefix dialect, see
/// [`NOT_IMPLEMENTED_PREFIX`]): a failed test whose every recorded issue
/// carries the generated-stub marker has no real step implementation and
/// maps to `Undefined`; any non-marker issue keeps it `Failed`.
///
/// # Errors
/// Malformed JSON lines, missing join fields, or a `testEnded` symbol
/// outside `pass`/`fail` are rejected loudly.
pub fn parse_swift_events_jsonl(input: &str) -> Result<Vec<ScenarioResult>, NormalizeError> {
    let mut events = SwiftEvents::default();
    for line in input.lines().filter(|l| !l.trim().is_empty()) {
        let record: Value = serde_json::from_str(line).map_err(|source| NormalizeError::Json {
            context: SWIFT_CTX,
            source,
        })?;
        events.ingest(&record)?;
    }
    Ok(events.into_results())
}

const SWIFT_CTX: &str = "Swift Testing event stream JSONL";

/// One recorded issue of a swift test. Shared with the xcresult parser in
/// `adapters::xcodebuild` — both ingestion paths speak the same
/// message-prefix dialect.
pub(crate) struct SwiftIssue {
    pub(crate) text: String,
    pub(crate) is_stub_marker: bool,
}

/// The message-prefix dialect verdict for a failed swift test: when every
/// recorded issue carries the generated-stub marker, no real step
/// implementation exists — [`Status::Undefined`], not Failed. Any
/// non-marker issue (or no issues at all) keeps it Failed.
pub(crate) fn swift_failed_status(issues: &[SwiftIssue]) -> Status {
    if !issues.is_empty() && issues.iter().all(|i| i.is_stub_marker) {
        Status::Undefined
    } else {
        Status::Failed
    }
}

/// Accumulator over the event stream, keyed by the 3-part test ID.
#[derive(Default)]
struct SwiftEvents {
    /// Discovery order drives output order: `(testID, displayName)`.
    functions: Vec<(String, String)>,
    /// suite id (`Target.Suite`) → feature name.
    suites: std::collections::HashMap<String, String>,
    /// testID → `instant.absolute` at start.
    started: std::collections::HashMap<String, f64>,
    /// testID → (passed, `instant.absolute` at end).
    ended: std::collections::HashMap<String, (bool, f64)>,
    issues: std::collections::HashMap<String, Vec<SwiftIssue>>,
}

impl SwiftEvents {
    fn ingest(&mut self, record: &Value) -> Result<(), NormalizeError> {
        let Some(payload) = record.get("payload") else {
            return Ok(());
        };
        let payload_kind = payload.get("kind").and_then(Value::as_str).unwrap_or("");
        match record.get("kind").and_then(Value::as_str) {
            Some("test") if payload_kind == "function" => {
                let id = str_field(payload, "id", SWIFT_CTX)?;
                let display = payload
                    .get("displayName")
                    .and_then(Value::as_str)
                    .map_or_else(
                        || str_field(payload, "name", SWIFT_CTX),
                        |d| Ok(d.to_owned()),
                    )?;
                self.functions.push((id, display));
            }
            Some("test") if payload_kind == "suite" => {
                let id = str_field(payload, "id", SWIFT_CTX)?;
                let display = payload
                    .get("displayName")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let feature = display.strip_prefix("Feature: ").unwrap_or(display);
                self.suites.insert(id, feature.to_owned());
            }
            Some("event") => self.ingest_event(payload, payload_kind)?,
            _ => {}
        }
        Ok(())
    }

    fn ingest_event(&mut self, payload: &Value, kind: &str) -> Result<(), NormalizeError> {
        let instant = payload
            .pointer("/instant/absolute")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        match kind {
            "testStarted" => {
                self.started
                    .insert(str_field(payload, "testID", SWIFT_CTX)?, instant);
            }
            "testEnded" => {
                let id = str_field(payload, "testID", SWIFT_CTX)?;
                let symbol = payload
                    .pointer("/messages/0/symbol")
                    .and_then(Value::as_str)
                    .ok_or(NormalizeError::MissingField {
                        field: "messages[0].symbol",
                        context: SWIFT_CTX,
                    })?;
                let passed = match symbol {
                    "pass" => true,
                    "fail" => false,
                    other => {
                        return Err(NormalizeError::UnknownStatus {
                            status: other.to_owned(),
                            context: SWIFT_CTX,
                        });
                    }
                };
                self.ended.insert(id, (passed, instant));
            }
            "issueRecorded" => {
                let id = str_field(payload, "testID", SWIFT_CTX)?;
                let texts: Vec<&str> = payload
                    .get("messages")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|m| m.get("text").and_then(Value::as_str))
                    .collect();
                let is_stub_marker = texts.iter().any(|t| t.starts_with(NOT_IMPLEMENTED_PREFIX));
                let mut text = texts.join(" — ");
                if let Some(row) = payload
                    .pointer("/_testCase/displayName")
                    .and_then(Value::as_str)
                {
                    text = format!("[{row}] {text}");
                }
                self.issues.entry(id).or_default().push(SwiftIssue {
                    text,
                    is_stub_marker,
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn into_results(mut self) -> Vec<ScenarioResult> {
        let functions = std::mem::take(&mut self.functions);
        functions
            .into_iter()
            .map(|(id, scenario)| {
                let feature = id
                    .split('/')
                    .next()
                    .and_then(|suite_id| self.suites.get(suite_id))
                    .cloned()
                    .unwrap_or_default();
                let test_issues = self.issues.remove(&id).unwrap_or_default();
                let (status, duration_ms) = match self.ended.get(&id) {
                    Some(&(true, at)) => (Status::Passed, self.duration_until(&id, at)),
                    Some(&(false, at)) => (
                        swift_failed_status(&test_issues),
                        self.duration_until(&id, at),
                    ),
                    // Discovered but never ended: the run died mid-test —
                    // never a pass.
                    None => (Status::Failed, None),
                };
                let failure = (status != Status::Passed).then(|| {
                    if test_issues.is_empty() {
                        "test did not finish (no testEnded event)".to_owned()
                    } else {
                        test_issues
                            .iter()
                            .map(|i| i.text.as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                });
                ScenarioResult {
                    feature,
                    scenario,
                    status,
                    duration_ms,
                    failure,
                }
            })
            .collect()
    }

    /// Milliseconds from the test's `testStarted` instant to `ended_at`.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "durations are small non-negative reals; clamped before the cast"
    )]
    fn duration_until(&self, id: &str, ended_at: f64) -> Option<u64> {
        self.started
            .get(id)
            .map(|s| ((ended_at - s).max(0.0) * 1000.0) as u64)
    }
}

// ---------------------------------------------------------------------------
// Parser 4: JUnit XML (pytest-bdd --junitxml, cucumber-rs output-junit, bats,
// swift-testing xunit sibling)
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
    /// The `*-swift-testing.xml` sibling `swift test --xunit-output` writes —
    /// a COARSE fallback only (ADR-001: no per-row results, no suite display
    /// names). testcase name = `` `Scenario name`(signature) `` — the raw
    /// identifier between backticks is the scenario; classname is the mangled
    /// `Target.Suite`, kept verbatim as the feature. UNDEFINED appears as a
    /// failure whose message carries the generated-stub marker.
    SwiftTesting,
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
                JunitDialect::SwiftTesting => {
                    if status == Status::Failed
                        && failure
                            .as_deref()
                            .is_some_and(|f| f.contains(NOT_IMPLEMENTED_PREFIX))
                    {
                        status = Status::Undefined;
                    }
                    (classname.to_owned(), raw_identifier_of(raw_name))
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

/// swift-testing xunit testcase names are `` `Scenario name`(signature) `` —
/// the SE-0451 raw identifier between the first and last backtick is the
/// scenario name verbatim. Names without backticks (non-generated tests)
/// pass through unchanged.
fn raw_identifier_of(raw: &str) -> String {
    match (raw.find('`'), raw.rfind('`')) {
        (Some(first), Some(last)) if last > first => raw[first + 1..last].to_owned(),
        _ => raw.to_owned(),
    }
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

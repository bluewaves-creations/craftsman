//! Parser 3: Swift Testing event stream JSONL v0 (ADR-001) —
//! `swift test --experimental-event-stream-output …
//! --experimental-event-stream-version 0`.

use serde_json::Value;

use super::{NOT_IMPLEMENTED_PREFIX, NormalizeError, ScenarioResult, Status, str_field};

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
pub struct SwiftIssue {
    pub text: String,
    pub is_stub_marker: bool,
}

/// The message-prefix dialect verdict for a failed swift test: when every
/// recorded issue carries the generated-stub marker, no real step
/// implementation exists — [`Status::Undefined`], not Failed. Any
/// non-marker issue (or no issues at all) keeps it Failed.
pub fn swift_failed_status(issues: &[SwiftIssue]) -> Status {
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

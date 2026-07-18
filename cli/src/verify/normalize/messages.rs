//! Parser 1: Cucumber Messages NDJSON (`@cucumber/cucumber --format message`).
//!
//! Joins gherkinDocument → pickle → testCase → testCaseStarted →
//! testStepFinished (the fixture-proven chain from ADR-002).

use std::collections::HashMap;

use serde_json::Value;

use super::{NormalizeError, ScenarioResult, Status, str_field, worst};

const CTX: &str = "Cucumber Messages NDJSON";

/// Parse a Messages NDJSON stream into normalized results.
///
/// # Errors
/// Any non-JSON line, missing join id, or unknown status is rejected.
pub fn parse_messages_ndjson(input: &str) -> Result<Vec<ScenarioResult>, NormalizeError> {
    let mut acc = Messages::default();
    for line in input.lines().filter(|l| !l.trim().is_empty()) {
        let m: Value = serde_json::from_str(line).map_err(|source| NormalizeError::Json {
            context: CTX,
            source,
        })?;
        acc.ingest(&m)?;
    }
    acc.into_results()
}

/// Accumulator over the message stream — the same shape as the swift
/// event-stream parser next door.
#[derive(Default)]
struct Messages {
    feature: String,
    /// pickleId -> scenario name.
    pickle_names: HashMap<String, String>,
    /// testCaseId -> pickleId.
    testcase_pickle: HashMap<String, String>,
    /// (testCaseStartedId, testCaseId) in start order.
    started_testcase: Vec<(String, String)>,
    /// testCaseStartedId -> (statuses, total nanos, first failure message).
    runs: HashMap<String, (Vec<Status>, u64, Option<String>)>,
}

impl Messages {
    fn ingest(&mut self, m: &Value) -> Result<(), NormalizeError> {
        if let Some(doc) = m.get("gherkinDocument") {
            if let Some(name) = doc.pointer("/feature/name").and_then(Value::as_str) {
                name.clone_into(&mut self.feature);
            }
        } else if let Some(p) = m.get("pickle") {
            self.pickle_names
                .insert(str_field(p, "id", CTX)?, str_field(p, "name", CTX)?);
        } else if let Some(tc) = m.get("testCase") {
            self.testcase_pickle
                .insert(str_field(tc, "id", CTX)?, str_field(tc, "pickleId", CTX)?);
        } else if let Some(tcs) = m.get("testCaseStarted") {
            self.started_testcase.push((
                str_field(tcs, "id", CTX)?,
                str_field(tcs, "testCaseId", CTX)?,
            ));
        } else if let Some(tsf) = m.get("testStepFinished") {
            self.ingest_step_finished(tsf)?;
        }
        Ok(())
    }

    fn ingest_step_finished(&mut self, tsf: &Value) -> Result<(), NormalizeError> {
        let run_id = str_field(tsf, "testCaseStartedId", CTX)?;
        let result = tsf
            .get("testStepResult")
            .ok_or(NormalizeError::MissingField {
                field: "testStepResult",
                context: CTX,
            })?;
        let status = Status::from_cucumber(&str_field(result, "status", CTX)?, CTX)?;
        let entry = self.runs.entry(run_id).or_default();
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
        Ok(())
    }

    fn into_results(mut self) -> Result<Vec<ScenarioResult>, NormalizeError> {
        let started = std::mem::take(&mut self.started_testcase);
        started
            .iter()
            .map(|(run_id, testcase_id)| {
                let scenario = self
                    .testcase_pickle
                    .get(testcase_id)
                    .and_then(|pid| self.pickle_names.get(pid))
                    .cloned()
                    .ok_or(NormalizeError::MissingField {
                        field: "pickle",
                        context: CTX,
                    })?;
                let (statuses, nanos, failure) = self.runs.remove(run_id).unwrap_or_default();
                Ok(ScenarioResult {
                    feature: self.feature.clone(),
                    scenario,
                    status: worst(statuses),
                    duration_ms: Some(nanos / 1_000_000),
                    failure,
                })
            })
            .collect()
    }
}

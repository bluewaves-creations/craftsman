//! Six-status result normalizer — schema v1, frozen by ADR-002.
//!
//! Four parsers, one submodule each (Cucumber Messages NDJSON,
//! cucumber-json, Swift Testing event-stream JSONL, `JUnit` XML), all
//! producing `Vec<ScenarioResult>`. `JUnit` needs a per-runner dialect because
//! plain `JUnit` cannot express UNDEFINED/PENDING — each runner smuggles that
//! information somewhere different (or nowhere: the pytest-bdd trap, where
//! cucumber-json silently omits undefined steps).
//!
//! Ported from `spikes/s2-normalizer` (fixture-proven skeletons kept
//! verbatim); error handling redone per repo conventions — malformed input
//! and unknown status strings are rejected loudly instead of being swallowed.

mod cucumber_json;
mod junit;
mod messages;
mod swift_events;

pub use cucumber_json::{CucumberJsonDialect, parse_cucumber_json};
pub use junit::{JunitDialect, parse_junit};
pub use messages::parse_messages_ndjson;
pub use swift_events::parse_swift_events_jsonl;
pub(crate) use swift_events::{SwiftIssue, swift_failed_status};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Cucumber Messages status vocabulary — the schema v1 status set.
///
/// Variant order doubles as severity: scenario status = `max` over step
/// statuses (Passed < Skipped < Pending < Undefined < Ambiguous < Failed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

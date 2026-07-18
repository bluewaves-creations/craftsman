//! Parser 2: cucumber-json (pytest-bdd `--cucumberjson`, cucumber-js
//! `--format json`, cucumber-rs `output-json`).

use serde_json::Value;

use super::{NormalizeError, ScenarioResult, Status, str_field, worst};

const CTX: &str = "cucumber-json";

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
            out.push(element_result(&feature, el, dialect)?);
        }
    }
    Ok(out)
}

/// One scenario element's normalized result.
fn element_result(
    feature: &str,
    el: &Value,
    dialect: CucumberJsonDialect,
) -> Result<ScenarioResult, NormalizeError> {
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
    Ok(ScenarioResult {
        feature: feature.to_owned(),
        scenario: str_field(el, "name", CTX)?,
        status: worst(statuses),
        duration_ms: Some(nanos / 1_000_000),
        failure,
    })
}

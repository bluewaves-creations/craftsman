//! Parser 4: `JUnit` XML (pytest-bdd `--junitxml`, cucumber-rs
//! `output-junit`, bats, swift-testing xunit sibling).
//!
//! `JUnit` itself only knows pass/failure/error/skipped; each runner needs
//! a dialect to recover the six-status vocabulary from runner-specific
//! markers — all empirically observed (ADR-002).

use super::{NOT_IMPLEMENTED_PREFIX, NormalizeError, ScenarioResult, Status};

const CTX: &str = "JUnit XML";

/// The per-runner dialect recovering UNDEFINED from `JUnit`'s vocabulary.
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
    let doc = roxmltree::Document::parse(input)?;
    let mut out = Vec::new();
    for suite in doc.descendants().filter(|n| n.has_tag_name("testsuite")) {
        let suite_name = suite.attribute("name").unwrap_or_default();
        for case in suite.children().filter(|n| n.has_tag_name("testcase")) {
            out.push(case_result(dialect, suite_name, case)?);
        }
    }
    Ok(out)
}

/// One testcase's normalized result under a dialect.
fn case_result(
    dialect: JunitDialect,
    suite_name: &str,
    case: roxmltree::Node<'_, '_>,
) -> Result<ScenarioResult, NormalizeError> {
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
    let system_out = child("system-out").map(text_of).unwrap_or_default();
    let (base_status, mut failure) = child("failure").or_else(|| child("error")).map_or_else(
        || {
            child("skipped").map_or((Status::Passed, None), |s| {
                (Status::Skipped, Some(text_of(s)).filter(|t| !t.is_empty()))
            })
        },
        |f| (Status::Failed, Some(text_of(f))),
    );

    let status = dialect_status(dialect, base_status, failure.as_deref(), &system_out);
    let (feature, scenario) = dialect_names(dialect, suite_name, classname, raw_name);
    if status == Status::Passed {
        failure = None;
    }
    Ok(ScenarioResult {
        feature,
        scenario,
        status,
        duration_ms,
        failure,
    })
}

/// Dialect quirk recovery: where each runner smuggles UNDEFINED.
fn dialect_status(
    dialect: JunitDialect,
    status: Status,
    failure: Option<&str>,
    system_out: &str,
) -> Status {
    let undefined = match dialect {
        JunitDialect::PytestBdd => {
            status == Status::Failed
                && failure.is_some_and(|f| f.contains("StepDefinitionNotFoundError"))
        }
        JunitDialect::CucumberRs => status == Status::Skipped && system_out.contains("?  "),
        JunitDialect::Bats => {
            status == Status::Skipped
                && failure.is_some_and(|f| f.starts_with("step not implemented"))
        }
        JunitDialect::SwiftTesting => {
            status == Status::Failed && failure.is_some_and(|f| f.contains(NOT_IMPLEMENTED_PREFIX))
        }
    };
    if undefined { Status::Undefined } else { status }
}

/// Feature/scenario names per dialect (each runner encodes them elsewhere).
fn dialect_names(
    dialect: JunitDialect,
    suite_name: &str,
    classname: &str,
    raw_name: &str,
) -> (String, String) {
    match dialect {
        JunitDialect::PytestBdd | JunitDialect::Bats => {
            let feature = if dialect == JunitDialect::PytestBdd {
                classname
            } else {
                suite_name
            };
            (feature.to_owned(), raw_name.to_owned())
        }
        JunitDialect::CucumberRs => (
            strip_cucumber_rs_name(suite_name, "Feature: "),
            strip_cucumber_rs_name(raw_name, "Scenario: "),
        ),
        JunitDialect::SwiftTesting => (classname.to_owned(), raw_identifier_of(raw_name)),
    }
}

/// A node's message attribute plus body text, joined and trimmed.
fn text_of(n: roxmltree::Node<'_, '_>) -> String {
    let attr_msg = n.attribute("message").unwrap_or_default();
    let body = n.text().unwrap_or_default();
    format!(
        "{attr_msg}{}{body}",
        if attr_msg.is_empty() { "" } else { "\n" }
    )
    .trim()
    .to_owned()
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

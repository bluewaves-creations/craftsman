//! xcodebuild adapter (swift stack) — the Apple variant, selected by
//! `[verify.swift] scheme` (Batch 9a; closes the Batch 5 honest-undone).
//!
//! Invocation: `xcodebuild test -scheme <s> -destination <d>
//! -resultBundlePath <artifacts>/xcodebuild.xcresult` in the package/project
//! directory. The destination defaults to `platform=macOS` so CI-less local
//! runs work without a simulator (probed on this machine: the macOS
//! destination builds and tests a `SwiftPM` package scheme in seconds; set
//! `[verify.swift] destination` for simulator runs).
//!
//! Fixture reality (probed, Xcode 26.6): xcodebuild tests a **`SwiftPM`
//! package scheme directly** — no `.xcodeproj` needed. `xcodebuild -list`
//! in a package directory shows a synthesized scheme named after the
//! package's product (or `<name>-Package` when it declares none);
//! `xcodebuild test -scheme <that>` builds and runs its test targets.
//! That synthesized scheme IS the committed fixture
//! (`cli/tests/fixtures/xcode-app/`); `swift package generate-xcodeproj`
//! is gone from modern `SwiftPM` and never needed.
//!
//! Selection (`-only-testing:`), verified empirically — the identifier is
//! `<Target>/<Suite>/`<name>`(<signature>)` where the backticks are
//! literal, the name is unescaped (spaces, quotes, unicode all pass
//! through), and the trailing signature is REQUIRED and must match
//! exactly: `()` for plain tests, the argument-label form
//! `(quantity:reason:)` for parameterized ones. Observed: omitting the
//! parens, or writing `()` for a parameterized test, silently matches 0
//! tests (exit 0, "Test run with 0 tests"). Craftsman therefore counts
//! matched tests itself; zero results become the dispatcher's exit 4.
//!
//! Verdict: exit 65 means "some tests failed" but is ambiguous (build
//! failures can share it) — the bundle is parsed regardless of the exit
//! code, and only a missing bundle is a runner failure. Results come from
//! `xcrun xcresulttool get test-results tests --path <bundle>` (Xcode 16+
//! subcommand; JSON shape captured from a real bundle into
//! `cli/tests/fixtures/xcresult-tests.json`): a `testNodes` tree of
//! `Test Suite` → `Test Case` → (`Arguments` rows / `Failure Message`)
//! nodes. Suite display names carry the generated `Feature: <name>` title;
//! test-case names are the scenario display names verbatim. Stub-marker
//! failures map to Undefined via the shared message-prefix dialect
//! (`normalize::swift_failed_status` — the same rule as the JSONL path).

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

use super::{AdapterError, tail};
use crate::verify::normalize::{
    NOT_IMPLEMENTED_PREFIX, NormalizeError, ScenarioResult, Status, SwiftIssue, swift_failed_status,
};

/// Default `-destination` when `[verify.swift] destination` is absent:
/// macOS needs no simulator, so local and CI-less runs stay cheap.
pub const DEFAULT_DESTINATION: &str = "platform=macOS";

/// Run `xcodebuild test` for `scheme` and normalize the result bundle.
///
/// `selectors` are full `-only-testing:` identifiers (see
/// [`only_testing_selector`]); `None` runs the whole scheme.
///
/// # Errors
/// [`AdapterError`] on spawn failure, a run that produced no result
/// bundle (build error), or a bundle xcresulttool cannot read.
pub fn run(
    project_dir: &Path,
    artifacts_dir: &Path,
    scheme: &str,
    destination: Option<&str>,
    selectors: Option<&[String]>,
) -> Result<Vec<ScenarioResult>, AdapterError> {
    std::fs::create_dir_all(artifacts_dir).map_err(|source| AdapterError::ResultsPath {
        path: artifacts_dir.to_path_buf(),
        source,
    })?;
    // xcodebuild refuses to overwrite an existing bundle: clear it first.
    let bundle = artifacts_dir.join("xcodebuild.xcresult");
    if bundle.exists() {
        std::fs::remove_dir_all(&bundle).map_err(|source| AdapterError::ResultsPath {
            path: bundle.clone(),
            source,
        })?;
    }

    let mut cmd = Command::new("xcodebuild");
    cmd.arg("test")
        .arg("-scheme")
        .arg(scheme)
        .arg("-destination")
        .arg(destination.unwrap_or(DEFAULT_DESTINATION))
        .arg("-resultBundlePath")
        .arg(&bundle);
    for selector in selectors.into_iter().flatten() {
        cmd.arg(format!("-only-testing:{selector}"));
    }
    let command_line = format!("xcodebuild test -scheme {scheme}");

    let output = cmd
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|source| AdapterError::Spawn {
            command: command_line.clone(),
            dir: project_dir.to_path_buf(),
            source,
        })?;
    // Runner stdout is progress, not verdict: forward it to stderr.
    eprint!("{}", String::from_utf8_lossy(&output.stdout));

    // Exit 65 is ambiguous (test failures AND some build failures share
    // it) — the bundle is the verdict wherever it exists. Observed: even a
    // scheme-not-found error writes a (test-less) bundle, so an empty
    // parse from a failing xcodebuild is a tool failure too — only a
    // clean exit may report zero tests (the -only-testing zero-match
    // case, which the dispatcher turns into exit 4).
    let runner_failed = |command: String| AdapterError::RunnerFailed {
        command,
        code: output
            .status
            .code()
            .map_or_else(|| "signal".to_owned(), |c| c.to_string()),
        output_tail: tail(&String::from_utf8_lossy(&output.stdout), 20),
    };
    if !bundle.exists() {
        return Err(runner_failed(command_line));
    }
    let json = extract_tests_json(&bundle)?;
    let results = parse_xcresult_tests(&json)?;
    if results.is_empty() && !output.status.success() {
        return Err(runner_failed(command_line));
    }
    Ok(results)
}

/// `xcrun xcresulttool get test-results tests --path <bundle>` — the
/// Xcode 16+ subcommand (flags verified via `--help` on Xcode 26.6).
fn extract_tests_json(bundle: &Path) -> Result<String, AdapterError> {
    let command_line = "xcrun xcresulttool get test-results tests".to_owned();
    let output = Command::new("xcrun")
        .args(["xcresulttool", "get", "test-results", "tests", "--path"])
        .arg(bundle)
        .output()
        .map_err(|source| AdapterError::Spawn {
            command: command_line.clone(),
            dir: bundle.to_path_buf(),
            source,
        })?;
    if !output.status.success() {
        return Err(AdapterError::RunnerFailed {
            command: command_line,
            code: output
                .status
                .code()
                .map_or_else(|| "signal".to_owned(), |c| c.to_string()),
            output_tail: tail(&String::from_utf8_lossy(&output.stderr), 20),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The verified `-only-testing:` identifier for one generated scenario.
///
/// Shape: `` <Target>/<Suite>/`<name>`(<signature>) `` — name verbatim
/// (never escaped), signature mandatory (module docs; `codegen::swift`
/// derives it from the scenario's Examples headers).
#[must_use]
pub fn only_testing_selector(
    test_target: &str,
    suite: &str,
    name: &str,
    signature: &str,
) -> String {
    format!("{test_target}/{suite}/`{name}`({signature})")
}

const XCRESULT_CTX: &str = "xcresulttool test-results JSON";

/// Parse the `test-results tests` JSON document (shape per the committed
/// fixture — see module docs) into normalized scenario results.
///
/// # Errors
/// Malformed JSON, a document without `testNodes`, or a test-case result
/// outside the schema's vocabulary.
pub fn parse_xcresult_tests(input: &str) -> Result<Vec<ScenarioResult>, NormalizeError> {
    let doc: Value = serde_json::from_str(input).map_err(|source| NormalizeError::Json {
        context: XCRESULT_CTX,
        source,
    })?;
    let nodes =
        doc.get("testNodes")
            .and_then(Value::as_array)
            .ok_or(NormalizeError::UnexpectedShape {
                context: XCRESULT_CTX,
                expected: "a top-level `testNodes` array",
            })?;
    let mut out = Vec::new();
    for node in nodes {
        collect_cases(node, "", &mut out)?;
    }
    Ok(out)
}

/// Walk the node tree: `Test Suite` display names set the feature (the
/// generated `Feature: <name>` title), `Test Case` nodes become results,
/// everything else (plan/bundle/config nodes) just recurses.
fn collect_cases(
    node: &Value,
    feature: &str,
    out: &mut Vec<ScenarioResult>,
) -> Result<(), NormalizeError> {
    let node_type = node.get("nodeType").and_then(Value::as_str).unwrap_or("");
    if node_type == "Test Case" {
        out.push(case_result(node, feature)?);
        return Ok(());
    }
    let feature = if node_type == "Test Suite" {
        let name = node.get("name").and_then(Value::as_str).unwrap_or_default();
        name.strip_prefix("Feature: ").unwrap_or(name)
    } else {
        feature
    };
    for child in node
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        collect_cases(child, feature, out)?;
    }
    Ok(())
}

/// One `Test Case` node → one normalized result.
fn case_result(node: &Value, feature: &str) -> Result<ScenarioResult, NormalizeError> {
    let scenario = node
        .get("name")
        .and_then(Value::as_str)
        .ok_or(NormalizeError::MissingField {
            field: "name",
            context: XCRESULT_CTX,
        })?
        .to_owned();
    let result =
        node.get("result")
            .and_then(Value::as_str)
            .ok_or(NormalizeError::MissingField {
                field: "result",
                context: XCRESULT_CTX,
            })?;
    let mut issues = Vec::new();
    collect_issues(node, None, &mut issues);
    // Schema vocabulary (verified via `--schema`): Passed, Failed,
    // Skipped, Expected Failure, unknown. `unknown` on a case means the
    // run died around it — never a pass (same rule as the JSONL path).
    let status = match result {
        "Passed" | "Expected Failure" => Status::Passed,
        "Skipped" => Status::Skipped,
        "Failed" => swift_failed_status(&issues),
        "unknown" => Status::Failed,
        other => {
            return Err(NormalizeError::UnknownStatus {
                status: other.to_owned(),
                context: XCRESULT_CTX,
            });
        }
    };
    let failure = (status != Status::Passed && status != Status::Skipped).then(|| {
        if issues.is_empty() {
            format!("test did not pass (bundle result: {result}, no failure message)")
        } else {
            issues
                .iter()
                .map(|i| i.text.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        }
    });
    Ok(ScenarioResult {
        feature: feature.to_owned(),
        scenario,
        status,
        duration_ms: duration_ms(node),
        failure,
    })
}

/// `Failure Message` descendants of a case, tagged with their `Arguments`
/// row label when nested under one (`[<row>] <message>`, mirroring the
/// JSONL parser's parameterized-row prefix).
fn collect_issues(node: &Value, row: Option<&str>, out: &mut Vec<SwiftIssue>) {
    for child in node
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let child_type = child.get("nodeType").and_then(Value::as_str).unwrap_or("");
        let name = child
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match child_type {
            "Failure Message" => out.push(SwiftIssue {
                text: row.map_or_else(|| name.to_owned(), |r| format!("[{r}] {name}")),
                is_stub_marker: name.contains(NOT_IMPLEMENTED_PREFIX),
            }),
            "Arguments" => collect_issues(child, Some(name), out),
            _ => collect_issues(child, row, out),
        }
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "durations are small non-negative reals; clamped before the cast"
)]
fn duration_ms(node: &Value) -> Option<u64> {
    node.get("durationInSeconds")
        .and_then(Value::as_f64)
        .map(|s| (s.max(0.0) * 1000.0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_testing_selector_matches_the_probed_syntax() {
        // Probed on Xcode 26.6 (module docs): backticks literal, name
        // verbatim, mandatory signature.
        assert_eq!(
            only_testing_selector(
                "SpecSpikeTests",
                "TodoManagementFeature",
                "Adding a todo shows it in the list",
                ""
            ),
            "SpecSpikeTests/TodoManagementFeature/`Adding a todo shows it in the list`()"
        );
        assert_eq!(
            only_testing_selector("T", "S", "Rejecting", "quantity:reason:"),
            "T/S/`Rejecting`(quantity:reason:)"
        );
    }

    #[test]
    fn zero_match_bundles_parse_to_zero_results() {
        // Probed: an -only-testing that matches nothing leaves a bare
        // Test Plan node with result "unknown" and no children — the
        // dispatcher turns 0 results into exit 4.
        let json = r#"{"devices": [], "testNodes": [
            {"name": "Fixture", "nodeType": "Test Plan", "result": "unknown"}
        ]}"#;
        assert!(parse_xcresult_tests(json).expect("parses").is_empty());
        assert!(parse_xcresult_tests("{}").is_err(), "testNodes is required");
    }
}

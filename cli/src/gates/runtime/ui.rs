//! The a11y/visual runners: user-land `XCUITest` accessibility audits via
//! xcodebuild, and Playwright suites filtered by test glob — invocation
//! plus report parsing.

use std::path::Path;

use serde_json::Value;

use super::super::{Finding, GateError, Severity, exec, tail};
use crate::config::Config;

/// The Apple a11y path: `xcodebuild test -scheme <s>
/// -only-testing:<ui-test-target>` — the target's user-land `XCUITest`s
/// call `performAccessibilityAudit()` (`spec gen --a11y-stub` emits the
/// template); any audit finding fails its test, and failed tests in the
/// result bundle become gate findings. Reuses the verify xcodebuild
/// adapter (a bare target name is a valid `-only-testing:` selector).
pub(super) fn run_xcuitest_audit(
    root: &Path,
    scheme: &str,
    target: &str,
    destination: Option<&str>,
) -> Result<Vec<Finding>, GateError> {
    let artifacts = root.join(".craftsman").join("cache").join("a11y");
    eprintln!("gate a11y: xcodebuild test -scheme {scheme} -only-testing:{target} …");
    let results = crate::verify::adapters::xcodebuild::run(
        root,
        &artifacts,
        scheme,
        destination,
        Some(&[target.to_owned()]),
    )
    .map_err(|e| GateError::ToolFailed {
        tool: "xcodebuild".to_owned(),
        code: "-".to_owned(),
        output: e.to_string(),
    })?;
    // An -only-testing selector matching nothing exits 0 with a test-less
    // bundle — a misnamed ui-test-target must never be a silent green.
    if results.is_empty() {
        return Err(GateError::ToolFailed {
            tool: "xcodebuild".to_owned(),
            code: "0".to_owned(),
            output: format!(
                "ui-test-target {target:?} matched no tests in scheme \
                 {scheme:?} — check [a11y] ui-test-target"
            ),
        });
    }
    Ok(audit_findings(&results, target))
}

/// Failed audit tests → findings (test name + failure message).
fn audit_findings(
    results: &[crate::verify::normalize::ScenarioResult],
    target: &str,
) -> Vec<Finding> {
    use crate::verify::normalize::Status;
    results
        .iter()
        .filter(|r| !matches!(r.status, Status::Passed | Status::Skipped))
        .map(|r| Finding {
            gate: "a11y",
            tool: "xcodebuild",
            rule: "accessibility-audit".to_owned(),
            file: format!("{target}/{}", r.scenario),
            line: None,
            message: format!(
                "audit test failed: {}: {}",
                r.scenario,
                r.failure
                    .as_deref()
                    .unwrap_or("no failure detail in bundle")
            ),
            severity: Severity::High,
        })
        .collect()
}

// -------------------------------------------------------------- playwright

pub(super) fn run_playwright(
    root: &Path,
    config: &Config,
    gate: &'static str,
    glob: &str,
) -> Result<Vec<Finding>, GateError> {
    let package = config.gates.tools.get("playwright").map_or_else(
        || "playwright".to_owned(),
        |version| format!("playwright@{version}"),
    );
    let argv = vec![
        "bunx".to_owned(),
        package,
        "test".to_owned(),
        glob.to_owned(),
        "--reporter=json".to_owned(),
    ];
    eprintln!("gate {gate}: playwright test {glob} (JSON reporter) …");
    let output = exec(&argv, root, &[])?;
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // 0 = green, 1 = failing tests (verdict in the report). Anything else
    // without a parseable report is a tool failure.
    let parsed = parse_playwright_report(&stdout, gate);
    match parsed {
        Ok(findings) if matches!(code, 0 | 1) => Ok(findings),
        _ => Err(GateError::ToolFailed {
            tool: "playwright".to_owned(),
            code: code.to_string(),
            output: tail(
                &format!("{stdout}{}", String::from_utf8_lossy(&output.stderr)),
                30,
            ),
        }),
    }
}

/// Parse Playwright's JSON reporter (`suites` nest recursively; each
/// `spec` carries `title`, `ok`, `file`, `line`); tested against a REAL
/// captured artifact (`tests/fixtures/runtime/playwright-report.json`,
/// playwright 1.61.1 against the static-site fixture, 2026-07-18).
fn parse_playwright_report(stdout: &str, gate: &'static str) -> Result<Vec<Finding>, GateError> {
    let doc: Value = serde_json::from_str(stdout.trim()).map_err(|e| GateError::Parse {
        tool: "playwright",
        detail: format!("invalid JSON report: {e}"),
    })?;
    if doc["suites"].is_null() {
        return Err(GateError::Parse {
            tool: "playwright",
            detail: "JSON report lacks `suites`".to_owned(),
        });
    }
    let mut findings = Vec::new();
    for suite in doc["suites"].as_array().unwrap_or(&Vec::new()) {
        collect_failed_specs(suite, gate, &mut findings);
    }
    Ok(findings)
}

fn collect_failed_specs(suite: &Value, gate: &'static str, findings: &mut Vec<Finding>) {
    for spec in suite["specs"].as_array().unwrap_or(&Vec::new()) {
        if spec["ok"] == true {
            continue;
        }
        findings.push(Finding {
            gate,
            tool: "playwright",
            rule: "failed-spec".to_owned(),
            file: spec["file"].as_str().unwrap_or_default().to_owned(),
            line: spec["line"].as_u64(),
            message: format!(
                "spec failed: {}",
                spec["title"].as_str().unwrap_or("unnamed spec")
            ),
            severity: Severity::High,
        });
    }
    for nested in suite["suites"].as_array().unwrap_or(&Vec::new()) {
        collect_failed_specs(nested, gate, findings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_findings_carry_test_name_and_failure() {
        use crate::verify::normalize::{ScenarioResult, Status};
        let result = |scenario: &str, status: Status, failure: Option<&str>| ScenarioResult {
            feature: "App".to_owned(),
            scenario: scenario.to_owned(),
            status,
            duration_ms: Some(3),
            failure: failure.map(str::to_owned),
        };
        // Shapes as parsed from the task-1 bundle format (the a11y path
        // reads the same xcresult tests JSON via the xcodebuild adapter).
        let results = vec![
            result("testAccessibilityAudit", Status::Passed, None),
            result(
                "testContrastAudit",
                Status::Failed,
                Some("Element has insufficient contrast"),
            ),
            result("testSkippedAudit", Status::Skipped, Some("XCTSkip")),
        ];
        let findings = audit_findings(&results, "AppUITests");
        assert_eq!(findings.len(), 1, "only failed tests are findings");
        assert_eq!(findings[0].gate, "a11y");
        assert_eq!(findings[0].rule, "accessibility-audit");
        assert_eq!(findings[0].file, "AppUITests/testContrastAudit");
        assert!(
            findings[0].message.contains("insufficient contrast"),
            "{}",
            findings[0].message
        );
        assert_eq!(findings[0].severity, Severity::High);
    }
    #[test]
    fn playwright_report_parses_real_artifact() {
        // REAL artifact: `bunx playwright test tests/a11y-broken.spec.ts
        // --reporter=json` captured 2026-07-18 against the static-site
        // fixture (the a11y red case — axe violations fail the spec).
        let json = include_str!("../../../tests/fixtures/runtime/playwright-report.json");
        let findings = parse_playwright_report(json, "a11y").expect("real artifact parses");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].gate, "a11y");
        assert_eq!(findings[0].rule, "failed-spec");
        assert_eq!(findings[0].file, "a11y-broken.spec.ts");
        assert_eq!(findings[0].line, Some(7));
        assert!(
            findings[0]
                .message
                .contains("no detectable a11y violations"),
            "{}",
            findings[0].message
        );
        assert!(parse_playwright_report("{}", "a11y").is_err(), "no suites");
    }
    #[test]
    fn playwright_report_collects_failed_specs_recursively() {
        // Constructed nesting probe (real reports nest one level; this
        // guards the recursive walk over deeper describe blocks).
        let json = r#"{
            "suites": [{
                "title": "a11y.spec.ts", "file": "a11y.spec.ts",
                "specs": [],
                "suites": [{
                    "title": "home page", "file": "a11y.spec.ts",
                    "specs": [
                        {"title": "has no axe violations", "ok": false,
                         "file": "a11y.spec.ts", "line": 12, "tests": []},
                        {"title": "has a title", "ok": true,
                         "file": "a11y.spec.ts", "line": 30, "tests": []}
                    ]
                }]
            }],
            "stats": {"expected": 1, "unexpected": 1}
        }"#;
        let findings = parse_playwright_report(json, "a11y").expect("parses");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, Some(12));
    }
}

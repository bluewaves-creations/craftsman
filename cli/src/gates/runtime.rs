//! `craftsman perf|a11y|visual` — thin orchestration over runtime tools.
//!
//! These gates run *user-land* configurations; craftsman owns invocation,
//! refusal, and verdict normalization — never the test content:
//!
//! - **perf** — `[perf] lighthouse-config` → `bunx @lhci/cli autorun`
//!   (failed assertions from `.lighthouseci/assertion-results.json`), OR
//!   `[perf] k6-script` → the pinned k6 binary with `--summary-export`
//!   (crossed thresholds from the summary metrics).
//! - **a11y** — web path: `bunx playwright test <[a11y] test-glob>` with
//!   the JSON reporter (axe-based specs are user-land); apple path
//!   (`[a11y] scheme` + `ui-test-target`, Batch 9a): `xcodebuild test
//!   -only-testing:<target>` running user-land `XCUITest` audits, failed
//!   tests read from the result bundle via the verify xcodebuild adapter.
//! - **visual** — same runner as web a11y, `[visual] test-glob`
//!   (screenshot specs).
//!
//! A gate whose config section is absent **refuses** with a clear message
//! (exit 3: "not configured — see --help") — an enabled-but-unconfigured
//! runtime gate must never pass silently.
//!
//! Playwright resolves through `bunx` (house rule: bun, never npx), which
//! prefers the project's locally installed playwright — the project's
//! lockfile is the version pin. `[gates.tools] playwright` forces a
//! version when set.

use std::path::Path;

use serde_json::Value;

use super::adapter;
use super::{Finding, GateError, GateOutcome, Severity, exec, lint, tail, tools};
use crate::config::{Config, GateMode};

/// Pinned Lighthouse CI version (`[gates.tools] lhci` overrides).
const LHCI_VERSION: &str = "0.15.1";

/// Run one runtime gate (`perf`, `a11y`, or `visual`).
///
/// # Errors
/// [`GateError::NotConfigured`] when the gate's config section is absent;
/// tool spawn/parse failures.
pub fn run(
    root: &Path,
    config: &Config,
    gate: &'static str,
    changed: Option<&[String]>,
    mode: GateMode,
) -> Result<GateOutcome, GateError> {
    let mut notes: Vec<String> = Vec::new();
    if changed.is_some() {
        notes.push(format!(
            "{gate}: --changed never narrows a runtime gate — running the \
             configured suite in full"
        ));
    }
    let (findings, tool): (Vec<Finding>, &'static str) = match gate {
        "perf" => run_perf(root, config)?,
        "a11y" => {
            let a11y = config.a11y.as_ref().ok_or_else(|| {
                not_configured("a11y", "test-glob (web) or scheme + ui-test-target (apple)")
            })?;
            match (&a11y.test_glob, &a11y.scheme, &a11y.ui_test_target) {
                (Some(glob), None, None) => {
                    (run_playwright(root, config, "a11y", glob)?, "playwright")
                }
                (None, Some(scheme), Some(target)) => (
                    run_xcuitest_audit(root, scheme, target, a11y.destination.as_deref())?,
                    "xcodebuild",
                ),
                // Config validation enforces web XOR apple, each complete.
                _ => unreachable!("[a11y] validation admits exactly one complete path"),
            }
        }
        "visual" => {
            let glob = config
                .visual
                .as_ref()
                .map(|c| c.test_glob.clone())
                .ok_or_else(|| not_configured("visual", "test-glob"))?;
            (run_playwright(root, config, "visual", &glob)?, "playwright")
        }
        other => unreachable!("not a runtime gate: {other}"),
    };
    lint::finish(root, gate, findings, notes, vec![tool], changed, mode)
}

fn not_configured(gate: &'static str, key: &str) -> GateError {
    GateError::NotConfigured {
        gate,
        hint: format!("add [{gate}] {key} = \"…\" to craftsman.toml"),
    }
}

// --------------------------------------------------------------------- perf

fn run_perf(root: &Path, config: &Config) -> Result<(Vec<Finding>, &'static str), GateError> {
    let perf = config
        .perf
        .as_ref()
        .ok_or_else(|| GateError::NotConfigured {
            gate: "perf",
            hint: "add [perf] with lighthouse-config (lhci autorun) or \
               k6-script (k6 thresholds) to craftsman.toml"
                .to_owned(),
        })?;
    match (&perf.lighthouse_config, &perf.k6_script) {
        (Some(lighthouse), _) => Ok((run_lhci(root, config, lighthouse)?, "lhci")),
        (None, Some(script)) => Ok((run_k6(root, config, script)?, "k6")),
        (None, None) => Err(GateError::NotConfigured {
            gate: "perf",
            hint: "[perf] must set lighthouse-config or k6-script".to_owned(),
        }),
    }
}

fn run_lhci(root: &Path, config: &Config, lighthouse: &str) -> Result<Vec<Finding>, GateError> {
    let version = config
        .gates
        .tools
        .get("lhci")
        .cloned()
        .unwrap_or_else(|| LHCI_VERSION.to_owned());
    let argv = vec![
        "bunx".to_owned(),
        format!("@lhci/cli@{version}"),
        "autorun".to_owned(),
        "--config".to_owned(),
        lighthouse.to_owned(),
    ];
    eprintln!("gate perf: lhci@{version} autorun ({lighthouse}) …");
    let output = exec(&argv, root, &[])?;
    let code = output.status.code().unwrap_or(-1);
    if code == 0 {
        return Ok(Vec::new());
    }
    // Non-zero: the verdict lives in the assertion results; without them
    // the tool failed (never a silent green).
    let results = root.join(".lighthouseci").join("assertion-results.json");
    let text = std::fs::read_to_string(&results).map_err(|_| GateError::ToolFailed {
        tool: "lhci".to_owned(),
        code: code.to_string(),
        output: tail(
            &format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
            30,
        ),
    })?;
    parse_lhci_assertions(&text)
}

/// Parse `.lighthouseci/assertion-results.json` — an array of assertion
/// outcomes (`name`, `auditId`, `level`, `expected`, `actual`, `passed`,
/// `url`, `operator`), per the Lighthouse CI assertion docs (constructed
/// sample in tests; not captured from a live site run).
fn parse_lhci_assertions(text: &str) -> Result<Vec<Finding>, GateError> {
    let doc: Value = serde_json::from_str(text).map_err(|e| GateError::Parse {
        tool: "lhci",
        detail: format!("invalid assertion-results.json: {e}"),
    })?;
    let items = doc.as_array().ok_or_else(|| GateError::Parse {
        tool: "lhci",
        detail: "expected a top-level array of assertion results".to_owned(),
    })?;
    Ok(items
        .iter()
        .filter(|a| a["passed"] != true)
        .map(|a| Finding {
            gate: "perf",
            tool: "lhci",
            rule: a["auditId"]
                .as_str()
                .or_else(|| a["name"].as_str())
                .unwrap_or("assertion")
                .to_owned(),
            file: a["url"].as_str().unwrap_or_default().to_owned(),
            line: None,
            message: format!(
                "expected {} {}, got {}",
                a["operator"].as_str().unwrap_or("<="),
                a["expected"],
                a["actual"]
            ),
            severity: if a["level"] == "warn" {
                Severity::Medium
            } else {
                Severity::High
            },
        })
        .collect())
}

fn run_k6(root: &Path, config: &Config, script: &str) -> Result<Vec<Finding>, GateError> {
    let tool = adapter::tool("k6").expect("k6 is in the adapter table");
    let version = config
        .gates
        .tools
        .get("k6")
        .cloned()
        .unwrap_or_else(|| tool.default_version.to_owned());
    let resolved = tools::resolve(tool, &version)?;
    let cache = root.join(".craftsman").join("cache");
    std::fs::create_dir_all(&cache).map_err(|source| GateError::Io {
        path: cache.clone(),
        source,
    })?;
    let summary = cache.join("k6-summary.json");
    let mut argv = resolved.argv.clone();
    argv.extend([
        "run".to_owned(),
        "--summary-export".to_owned(),
        summary.to_string_lossy().into_owned(),
        script.to_owned(),
    ]);
    eprintln!("gate perf: k6 run ({script}) via {} …", resolved.via);
    let output = exec(&argv, root, &[])?;
    let code = output.status.code().unwrap_or(-1);
    if !tool.success_codes.contains(&code) {
        return Err(GateError::ToolFailed {
            tool: "k6".to_owned(),
            code: code.to_string(),
            output: tail(&String::from_utf8_lossy(&output.stderr), 30),
        });
    }
    let text = std::fs::read_to_string(&summary).map_err(|source| GateError::Io {
        path: summary,
        source,
    })?;
    parse_k6_summary(&text, script)
}

/// Parse a k6 `--summary-export` document: `metrics.<name>.thresholds.
/// <expr>` is `false` (k6 < 0.45 style) or `{"ok": false}` when crossed
/// (per the k6 end-of-test summary docs; constructed sample in tests).
fn parse_k6_summary(text: &str, script: &str) -> Result<Vec<Finding>, GateError> {
    let doc: Value = serde_json::from_str(text).map_err(|e| GateError::Parse {
        tool: "k6",
        detail: format!("invalid summary export: {e}"),
    })?;
    let metrics = doc["metrics"].as_object().ok_or_else(|| GateError::Parse {
        tool: "k6",
        detail: "summary export lacks a `metrics` object".to_owned(),
    })?;
    let mut findings = Vec::new();
    for (metric, entry) in metrics {
        let Some(thresholds) = entry["thresholds"].as_object() else {
            continue;
        };
        for (expr, verdict) in thresholds {
            let crossed = match verdict {
                Value::Bool(ok) => !ok,
                other => other["ok"] == false,
            };
            if crossed {
                findings.push(Finding {
                    gate: "perf",
                    tool: "k6",
                    rule: format!("{metric}:{expr}"),
                    file: script.to_owned(),
                    line: None,
                    message: format!("k6 threshold crossed: {metric} {expr}"),
                    severity: Severity::High,
                });
            }
        }
    }
    Ok(findings)
}

// ----------------------------------------------------- xcodebuild (a11y)

/// The Apple a11y path: `xcodebuild test -scheme <s>
/// -only-testing:<ui-test-target>` — the target's user-land `XCUITest`s
/// call `performAccessibilityAudit()` (`spec gen --a11y-stub` emits the
/// template); any audit finding fails its test, and failed tests in the
/// result bundle become gate findings. Reuses the verify xcodebuild
/// adapter (a bare target name is a valid `-only-testing:` selector).
fn run_xcuitest_audit(
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

fn run_playwright(
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
/// `spec` carries `title`, `ok`, `file`, `line` — per the Playwright
/// reporter docs; constructed sample in tests).
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
    fn unconfigured_runtime_gates_refuse_loudly() {
        let config = crate::config::Config::from_toml(
            "[project]\nname = \"x\"\nstacks = [\"rust\"]\n",
            Path::new("craftsman.toml"),
        )
        .expect("parses");
        for gate in ["perf", "a11y", "visual"] {
            let err = run(Path::new("."), &config, gate, None, GateMode::Strict)
                .expect_err("must refuse");
            assert!(
                matches!(err, GateError::NotConfigured { gate: g, .. } if g == gate),
                "{err}"
            );
            assert!(err.to_string().contains("not configured"), "{err}");
        }
    }

    #[test]
    fn perf_with_an_empty_section_refuses() {
        let config = crate::config::Config::from_toml(
            "[project]\nname = \"x\"\nstacks = [\"rust\"]\n[perf]\n",
            Path::new("craftsman.toml"),
        )
        .expect("parses");
        let err =
            run(Path::new("."), &config, "perf", None, GateMode::Strict).expect_err("must refuse");
        assert!(matches!(err, GateError::NotConfigured { .. }), "{err}");
    }

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
    fn lhci_assertion_results_parse() {
        // Constructed per the Lighthouse CI assertion docs (assert.md):
        // autorun writes an array of per-assertion outcomes.
        let json = r#"[
            {"name": "maxNumericValue", "expected": 2000, "actual": 3502.7,
             "values": [3502.7], "operator": "<=", "passed": false,
             "auditId": "largest-contentful-paint", "level": "error",
             "url": "http://localhost:3000/"},
            {"name": "minScore", "expected": 0.9, "actual": 0.95,
             "operator": ">=", "passed": true, "auditId": "performance",
             "level": "error", "url": "http://localhost:3000/"},
            {"name": "maxLength", "expected": 0, "actual": 1,
             "operator": "<=", "passed": false, "auditId": "unused-javascript",
             "level": "warn", "url": "http://localhost:3000/"}
        ]"#;
        let findings = parse_lhci_assertions(json).expect("parses");
        assert_eq!(findings.len(), 2, "passing assertions are not findings");
        assert_eq!(findings[0].rule, "largest-contentful-paint");
        assert_eq!(findings[0].severity, Severity::High);
        assert!(findings[0].message.contains("<= 2000"));
        assert_eq!(findings[1].severity, Severity::Medium, "warn level");
        assert!(
            parse_lhci_assertions("{}").is_err(),
            "object is not the format"
        );
    }

    #[test]
    fn k6_summary_thresholds_parse() {
        // Constructed per the k6 --summary-export docs: thresholds appear
        // as {"expr": false} (legacy) or {"expr": {"ok": false}}.
        let json = r#"{
            "metrics": {
                "http_req_duration": {
                    "avg": 120.5, "p(95)": 310.2,
                    "thresholds": {"p(95)<200": {"ok": false}}
                },
                "http_req_failed": {
                    "value": 0.001,
                    "thresholds": {"rate<0.01": {"ok": true}}
                },
                "iterations": {"count": 100}
            }
        }"#;
        let findings = parse_k6_summary(json, "load.js").expect("parses");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "http_req_duration:p(95)<200");
        assert_eq!(findings[0].file, "load.js");

        let legacy = r#"{"metrics": {"checks": {"thresholds": {"rate>0.9": false}}}}"#;
        let findings = parse_k6_summary(legacy, "s.js").expect("parses");
        assert_eq!(findings.len(), 1, "legacy boolean form");
        assert!(parse_k6_summary("[]", "s.js").is_err());
    }

    #[test]
    fn playwright_report_collects_failed_specs_recursively() {
        // Constructed per the Playwright JSON reporter docs (nested
        // suites; spec.ok carries the verdict).
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
        assert_eq!(findings[0].gate, "a11y");
        assert_eq!(findings[0].line, Some(12));
        assert!(findings[0].message.contains("axe"));
        assert!(parse_playwright_report("{}", "a11y").is_err(), "no suites");
    }
}

//! The perf gate's runners: Lighthouse CI (`lhci autorun`) and k6
//! thresholds — invocation plus report parsing.

use std::path::Path;

use serde_json::Value;

use super::super::{Finding, GateError, Severity, adapter, exec, tail, tools};
use super::LHCI_VERSION;
use crate::config::Config;

pub(super) fn run_perf(
    root: &Path,
    config: &Config,
) -> Result<(Vec<Finding>, &'static str), GateError> {
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
/// `url`, `operator`); tested against a REAL captured artifact
/// (`tests/fixtures/runtime/lhci-assertion-results.json`, lhci 0.15.1
/// against the static-site fixture, 2026-07-18).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lhci_assertion_results_parse_real_artifact() {
        // REAL artifact: .lighthouseci/assertion-results.json captured
        // 2026-07-18 from `bunx @lhci/cli@0.15.1 autorun --config
        // lighthouserc-strict.json` against the static-site fixture (the
        // perf red case: total-byte-weight <= 1 byte over 2 URLs).
        let json = include_str!("../../../tests/fixtures/runtime/lhci-assertion-results.json");
        let findings = parse_lhci_assertions(json).expect("real artifact parses");
        assert_eq!(findings.len(), 2, "one failed assertion per audited URL");
        assert_eq!(findings[0].rule, "total-byte-weight");
        assert_eq!(findings[0].severity, Severity::High);
        assert!(
            findings[0].message.contains("<= 1"),
            "{}",
            findings[0].message
        );
        assert!(
            findings[0].file.starts_with("http://localhost"),
            "url field"
        );
        assert!(
            parse_lhci_assertions("{}").is_err(),
            "object is not the format"
        );
    }
    #[test]
    fn k6_summary_thresholds_parse() {
        // Constructed per the k6 --summary-export docs: thresholds appear
        // as {"expr": false} (legacy) or {"expr": {"ok": false}}. Still
        // constructed after Batch 9b (k6 is the one runtime tool with no
        // live fixture run — the lhci path covers perf live).
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
}

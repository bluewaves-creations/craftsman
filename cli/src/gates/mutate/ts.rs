//! The typescript mutate runner: Stryker incremental through bunx;
//! verdicts from the mutation-report JSON.

use std::path::Path;

use serde_json::Value;

use super::super::{Finding, GateError, Severity, exec, tail};
use super::{
    Config, STRYKER_VERSION, Scope, StackRun, Tally, changed_stack_files, pinned, scope_word,
};

pub(super) fn ts_mutate(
    root: &Path,
    config: &Config,
    cwd: Option<&str>,
    scope: Scope,
) -> Result<StackRun, GateError> {
    let version = pinned(config, "stryker", STRYKER_VERSION);
    let dir = cwd.map_or_else(|| root.to_path_buf(), |c| root.join(c));
    let mut argv = vec![
        "bunx".to_owned(),
        format!("@stryker-mutator/core@{version}"),
        "run".to_owned(),
        "--incremental".to_owned(),
        "--reporters".to_owned(),
        "json".to_owned(),
    ];
    if scope == Scope::Diff {
        let changed = changed_stack_files(root, cwd, &["ts", "tsx", "js", "jsx"])?;
        if changed.is_empty() {
            return Ok(StackRun::skipped(
                "stryker",
                "mutate[typescript]: no changed source files — nothing to mutate".to_owned(),
            ));
        }
        argv.push("--mutate".to_owned());
        argv.push(changed.join(","));
    }
    eprintln!("gate mutate: stryker@{version} ({}) …", scope_word(scope));
    let output = exec(&argv, &dir, &[])?;
    let code = output.status.code().unwrap_or(-1);
    // Stryker exits 1 when its own break threshold trips; the report still
    // carries the verdict. Anything else is a tool failure.
    let report_path = dir.join("reports").join("mutation").join("mutation.json");
    if !matches!(code, 0 | 1) || !report_path.is_file() {
        return Err(GateError::ToolFailed {
            tool: "stryker".to_owned(),
            code: code.to_string(),
            output: tail(
                &format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
                30,
            ),
        });
    }
    let text = std::fs::read_to_string(&report_path).map_err(|source| GateError::Io {
        path: report_path,
        source,
    })?;
    let (tally, findings) = parse_stryker_report(&text, cwd)?;
    Ok(StackRun {
        tool: "stryker",
        tally,
        findings,
        note: None,
    })
}

/// Parse a mutation-testing-report-schema document (Stryker's JSON
/// reporter; shape per the official schema:
/// `files.<path>.mutants[].{status, location.start.line, mutatorName,
/// replacement}`). Killed/Timeout are caught; Survived/NoCoverage missed;
/// CompileError/Ignored excluded from the score.
fn parse_stryker_report(text: &str, cwd: Option<&str>) -> Result<(Tally, Vec<Finding>), GateError> {
    let doc: Value = serde_json::from_str(text).map_err(|e| GateError::Parse {
        tool: "stryker",
        detail: format!("invalid mutation report: {e}"),
    })?;
    let files = doc["files"].as_object().ok_or_else(|| GateError::Parse {
        tool: "stryker",
        detail: "mutation report lacks a `files` object".to_owned(),
    })?;
    let mut tally = Tally::default();
    let mut findings = Vec::new();
    for (path, entry) in files {
        let file = cwd.map_or_else(|| path.clone(), |c| format!("{c}/{path}"));
        for mutant in entry["mutants"].as_array().unwrap_or(&Vec::new()) {
            match mutant["status"].as_str().unwrap_or_default() {
                "Killed" => tally.caught += 1,
                "Timeout" => tally.timeout += 1,
                "Survived" | "NoCoverage" => {
                    tally.missed += 1;
                    findings.push(Finding {
                        gate: "mutate",
                        tool: "stryker",
                        rule: "survived-mutant".to_owned(),
                        file: file.clone(),
                        line: mutant["location"]["start"]["line"].as_u64(),
                        message: format!(
                            "{}: mutant survived ({})",
                            mutant["mutatorName"].as_str().unwrap_or("mutant"),
                            mutant["replacement"].as_str().unwrap_or("replacement")
                        ),
                        severity: Severity::Medium,
                    });
                }
                _ => tally.unviable += 1,
            }
        }
    }
    Ok((tally, findings))
}

// ------------------------------------------------------------------ helpers

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stryker_report_parses_per_schema() {
        // Constructed per the mutation-testing-report-schema (Stryker's
        // documented JSON reporter format) — not captured from a live run.
        let json = r#"{
            "schemaVersion": "2",
            "thresholds": {"high": 80, "low": 60},
            "files": {
                "src/calc.ts": {
                    "language": "typescript",
                    "mutants": [
                        {"id": "1", "mutatorName": "ArithmeticOperator", "replacement": "a - b",
                         "status": "Killed", "location": {"start": {"line": 2, "column": 10}, "end": {"line": 2, "column": 15}}},
                        {"id": "2", "mutatorName": "ConditionalExpression", "replacement": "false",
                         "status": "Survived", "location": {"start": {"line": 5, "column": 3}, "end": {"line": 5, "column": 9}}},
                        {"id": "3", "mutatorName": "BlockStatement", "replacement": "{}",
                         "status": "NoCoverage", "location": {"start": {"line": 9, "column": 1}, "end": {"line": 11, "column": 2}}},
                        {"id": "4", "mutatorName": "StringLiteral", "replacement": "\"\"",
                         "status": "CompileError", "location": {"start": {"line": 12, "column": 1}, "end": {"line": 12, "column": 5}}}
                    ]
                }
            }
        }"#;
        let (tally, findings) = parse_stryker_report(json, None).expect("parses");
        assert_eq!(tally.caught, 1);
        assert_eq!(tally.missed, 2);
        assert_eq!(tally.unviable, 1);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "src/calc.ts");
        assert_eq!(findings[0].line, Some(5));
    }
}

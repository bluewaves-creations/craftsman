//! Parsers for the lint-gate tools: cargo fmt/clippy, ruff, biome,
//! swiftlint, shellcheck — each mapping one tool's documented output into
//! normalized findings.

use serde_json::Value;

use super::super::{Finding, GateError, Severity};
use super::{finding, json_value};

/// `cargo fmt --check`: lines of `Diff in <file>:<line>:` (observed
/// rustfmt 1.8+; older toolchains write `Diff in <file> at line <line>:`).
pub(super) fn parse_cargo_fmt(stdout: &str, stderr: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for line in stdout.lines().chain(stderr.lines()) {
        let Some(rest) = line.strip_prefix("Diff in ") else {
            continue;
        };
        let rest = rest.trim_end_matches(':');
        let (file, line_no) = rest.rsplit_once(" at line ").map_or_else(
            || {
                rest.rsplit_once(':')
                    .map_or((rest, None), |(f, n)| (f, n.parse::<u64>().ok()))
            },
            |(f, n)| (f, n.parse::<u64>().ok()),
        );
        findings.push(finding(
            "lint",
            "fmt",
            "rustfmt",
            file,
            line_no,
            "rustfmt would reformat this file",
            Severity::Low,
        ));
    }
    findings
}

/// `cargo clippy --message-format=json`: JSON Lines; findings are
/// `compiler-message` records with a primary span. Duplicate diagnostics
/// (lib vs test target compilations) are deduplicated.
pub(super) fn parse_cargo_clippy(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let mut findings: Vec<Finding> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let doc: Value = serde_json::from_str(line).map_err(|e| GateError::Parse {
            tool: "clippy",
            detail: format!("invalid JSON line: {e}"),
        })?;
        if doc["reason"] != "compiler-message" {
            continue;
        }
        let msg = &doc["message"];
        let level = msg["level"].as_str().unwrap_or_default();
        let severity = match level {
            "warning" => Severity::Medium,
            "error" | "error: internal compiler error" => Severity::High,
            _ => continue,
        };
        let Some(span) = msg["spans"]
            .as_array()
            .and_then(|s| s.iter().find(|sp| sp["is_primary"] == true))
        else {
            continue; // summary records ("N warnings emitted") have no span
        };
        let rule = msg["code"]["code"].as_str().unwrap_or(level).to_owned();
        let file = span["file_name"].as_str().unwrap_or_default().to_owned();
        let line_no = span["line_start"].as_u64();
        let message = msg["message"].as_str().unwrap_or_default().to_owned();
        let key = format!("{rule}\x1f{file}\x1f{line_no:?}\x1f{message}");
        if seen.insert(key) {
            findings.push(finding(
                "lint", "clippy", rule, file, line_no, message, severity,
            ));
        }
    }
    Ok(findings)
}

/// `ruff check --output-format json`: an array of violations.
pub(super) fn parse_ruff(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("ruff", stdout)?;
    let items = doc.as_array().ok_or_else(|| GateError::Parse {
        tool: "ruff",
        detail: "expected a top-level array".to_owned(),
    })?;
    Ok(items
        .iter()
        .map(|v| {
            finding(
                "lint",
                "ruff",
                v["code"].as_str().unwrap_or("ruff"),
                v["filename"].as_str().unwrap_or_default(),
                v["location"]["row"].as_u64(),
                v["message"].as_str().unwrap_or_default(),
                Severity::Medium,
            )
        })
        .collect())
}

/// `ruff format --check`: plain lines `Would reformat: <file>`.
pub(super) fn parse_ruff_format(stdout: &str) -> Vec<Finding> {
    stdout
        .lines()
        .filter_map(|l| l.strip_prefix("Would reformat: "))
        .map(|file| {
            finding(
                "lint",
                "ruff-format",
                "ruff-format",
                file,
                None,
                "ruff format would reformat this file",
                Severity::Low,
            )
        })
        .collect()
}

/// `biome check --reporter=json`: `{"diagnostics": [...]}` with byte-span
/// locations. Line numbers are re-derived from the span: each diagnostic
/// carries its file's full text in `location.sourceCode` (observed live
/// against biome 2.2.5 — `tests/fixtures/biome-report.json`), so counting
/// newlines up to the span start needs no separate file read. A diagnostic
/// without a span or source text keeps `line: None` (never a guess).
pub(super) fn parse_biome(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("biome", stdout)?;
    let diags = doc["diagnostics"]
        .as_array()
        .ok_or_else(|| GateError::Parse {
            tool: "biome",
            detail: "expected a `diagnostics` array".to_owned(),
        })?;
    Ok(diags
        .iter()
        .map(|d| {
            let severity = match d["severity"].as_str().unwrap_or_default() {
                "error" | "fatal" => Severity::High,
                "warning" => Severity::Medium,
                _ => Severity::Info,
            };
            let file = d["location"]["path"]["file"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            let line = d["location"]["span"][0].as_u64().and_then(|offset| {
                d["location"]["sourceCode"]
                    .as_str()
                    .map(|source| line_at_byte_offset(source, offset))
            });
            finding(
                "lint",
                "biome",
                d["category"].as_str().unwrap_or("biome"),
                file,
                line,
                d["description"].as_str().unwrap_or_default(),
                severity,
            )
        })
        .collect())
}

/// 1-based line number of a byte offset into `source` — newline bytes
/// counted up to (excluding) the offset, clamped to the text length.
fn line_at_byte_offset(source: &str, offset: u64) -> u64 {
    let end = usize::try_from(offset).map_or(source.len(), |o| o.min(source.len()));
    // split() counts newline separators without the naive-bytecount lint
    // (a bytecount dependency is not worth diagnostic-sized texts).
    let newlines = source.as_bytes()[..end].split(|b| *b == b'\n').count() - 1;
    u64::try_from(newlines).unwrap_or(u64::MAX) + 1
}

/// `swiftlint lint --reporter json`: an array of violations.
pub(super) fn parse_swiftlint(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("swiftlint", stdout)?;
    let items = doc.as_array().ok_or_else(|| GateError::Parse {
        tool: "swiftlint",
        detail: "expected a top-level array".to_owned(),
    })?;
    Ok(items
        .iter()
        .map(|v| {
            let severity = match v["severity"].as_str().unwrap_or_default() {
                "Error" | "error" => Severity::High,
                _ => Severity::Medium,
            };
            finding(
                "lint",
                "swiftlint",
                v["rule_id"].as_str().unwrap_or("swiftlint"),
                v["file"].as_str().unwrap_or_default(),
                v["line"].as_u64(),
                v["reason"].as_str().unwrap_or_default(),
                severity,
            )
        })
        .collect())
}

/// `shellcheck --format json`: an array of comments.
pub(super) fn parse_shellcheck(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("shellcheck", stdout)?;
    let items = doc.as_array().ok_or_else(|| GateError::Parse {
        tool: "shellcheck",
        detail: "expected a top-level array".to_owned(),
    })?;
    Ok(items
        .iter()
        .map(|v| {
            let severity = match v["level"].as_str().unwrap_or_default() {
                "error" => Severity::High,
                "warning" => Severity::Medium,
                _ => Severity::Info,
            };
            let rule = v["code"]
                .as_u64()
                .map_or_else(|| "shellcheck".to_owned(), |c| format!("SC{c}"));
            finding(
                "lint",
                "shellcheck",
                rule,
                v["file"].as_str().unwrap_or_default(),
                v["line"].as_u64(),
                v["message"].as_str().unwrap_or_default(),
                severity,
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_fmt_diff_lines_become_findings() {
        let out = "Diff in /repo/cli/src/lib.rs:14:\n-bad\n+good\nDiff in /repo/cli/src/main.rs at line 3:\n";
        let f = parse_cargo_fmt(out, "");
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].file, "/repo/cli/src/lib.rs");
        assert_eq!(f[0].line, Some(14));
        assert_eq!(f[1].file, "/repo/cli/src/main.rs");
        assert_eq!(f[1].line, Some(3));
        assert!(parse_cargo_fmt("", "").is_empty());
    }

    #[test]
    fn clippy_json_lines_become_deduplicated_findings() {
        let line = r#"{"reason":"compiler-message","message":{"code":{"code":"clippy::needless_return"},"level":"warning","message":"unneeded `return` statement","spans":[{"is_primary":true,"file_name":"src/lib.rs","line_start":7}]}}"#;
        let summary = r#"{"reason":"compiler-message","message":{"code":null,"level":"warning","message":"1 warning emitted","spans":[]}}"#;
        let out = format!(
            "{line}\n{line}\n{summary}\n{{\"reason\":\"build-finished\",\"success\":true}}"
        );
        let f = parse_cargo_clippy(&out).expect("parses");
        assert_eq!(f.len(), 1, "duplicates and span-less summaries drop");
        assert_eq!(f[0].rule, "clippy::needless_return");
        assert_eq!(f[0].line, Some(7));
        assert_eq!(f[0].severity, Severity::Medium);
    }

    #[test]
    fn ruff_json_and_format_lines_parse() {
        let checked = r#"[{"code":"F401","filename":"app.py","location":{"row":1,"column":8},"message":"`os` imported but unused"}]"#;
        let f = parse_ruff(checked).expect("parses");
        assert_eq!(f[0].rule, "F401");
        assert_eq!(f[0].line, Some(1));
        let fmt = parse_ruff_format("Would reformat: app.py\n1 file would be reformatted\n");
        assert_eq!(fmt.len(), 1);
        assert_eq!(fmt[0].file, "app.py");
    }

    #[test]
    fn biome_swiftlint_shellcheck_parse() {
        let biome = r#"{"summary":{},"diagnostics":[{"category":"lint/suspicious/noDoubleEquals","severity":"error","description":"Use === instead of ==","location":{"path":{"file":"src/a.ts"}}}]}"#;
        let f = parse_biome(biome).expect("parses");
        assert_eq!(f[0].rule, "lint/suspicious/noDoubleEquals");
        assert_eq!(f[0].severity, Severity::High);
        assert_eq!(f[0].line, None, "no span/source → no invented line");

        let swift = r#"[{"rule_id":"line_length","file":"/app/A.swift","line":12,"reason":"Line too long","severity":"Warning"}]"#;
        let f = parse_swiftlint(swift).expect("parses");
        assert_eq!(f[0].rule, "line_length");
        assert_eq!(f[0].severity, Severity::Medium);

        let sc = r#"[{"file":"run.sh","line":3,"level":"warning","code":2086,"message":"Double quote to prevent globbing."}]"#;
        let f = parse_shellcheck(sc).expect("parses");
        assert_eq!(f[0].rule, "SC2086");
        assert_eq!(f[0].line, Some(3));
    }

    #[test]
    fn biome_lines_derive_from_byte_spans_real_artifact() {
        let report = include_str!("../../../tests/fixtures/biome-report.json");
        let f = parse_biome(report).expect("real report parses");
        assert_eq!(f.len(), 3);
        // `unused` sits at byte 4 of second.ts line 1.
        assert_eq!((f[0].file.as_str(), f[0].line), ("second.ts", Some(1)));
        // `y` of `var y == 2;` is byte 17 of bad.ts — line 2.
        assert_eq!(f[1].rule, "lint/suspicious/noImplicitAnyLet");
        assert_eq!((f[1].file.as_str(), f[1].line), ("bad.ts", Some(2)));
        // `debugger` starts at byte 41 — line 4.
        assert_eq!(f[2].rule, "lint/suspicious/noDebugger");
        assert_eq!((f[2].file.as_str(), f[2].line), ("bad.ts", Some(4)));
    }

    #[test]
    fn byte_offset_line_math_clamps() {
        assert_eq!(line_at_byte_offset("a\nb\nc", 0), 1);
        assert_eq!(line_at_byte_offset("a\nb\nc", 2), 2);
        assert_eq!(line_at_byte_offset("a\nb\nc", 999), 3, "clamped to len");
        assert_eq!(line_at_byte_offset("", 0), 1);
    }
}

//! The rust mutate runner: cargo-mutants `--in-diff`, hermetically
//! installed via `cargo install`; verdicts from `mutants.out/outcomes.json`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::super::{Finding, GateError, Severity, exec, tail, tools};
use super::{CARGO_MUTANTS_VERSION, Config, Scope, StackRun, Tally, pinned, scope_word};

pub(super) fn rust_mutate(
    root: &Path,
    config: &Config,
    cwd: Option<&str>,
    scope: Scope,
) -> Result<StackRun, GateError> {
    let version = pinned(config, "cargo-mutants", CARGO_MUTANTS_VERSION);
    let bin = resolve_cargo_mutants(&version)?;
    let dir = cwd.map_or_else(|| root.to_path_buf(), |c| root.join(c));
    let cache = root.join(".craftsman").join("cache");
    std::fs::create_dir_all(&cache).map_err(|source| GateError::Io {
        path: cache.clone(),
        source,
    })?;

    let mut argv = vec![
        bin.to_string_lossy().into_owned(),
        "mutants".to_owned(),
        "--no-shuffle".to_owned(),
        "--output".to_owned(),
        cache.join("mutants").to_string_lossy().into_owned(),
    ];
    if scope == Scope::Diff {
        let diff = stack_diff(root, cwd)?;
        if diff.trim().is_empty() {
            return Ok(StackRun::skipped(
                "cargo-mutants",
                "mutate[rust]: no tracked changes against HEAD — nothing to mutate".to_owned(),
            ));
        }
        let diff_path = cache.join("mutate-rust.diff");
        std::fs::write(&diff_path, &diff).map_err(|source| GateError::Io {
            path: diff_path.clone(),
            source,
        })?;
        argv.push("--in-diff".to_owned());
        argv.push(diff_path.to_string_lossy().into_owned());
    }
    // Unit tests only: the mutants build tree is a copy of the package,
    // where integration tests reading outside it cannot pass (module docs).
    argv.extend(["--".to_owned(), "--lib".to_owned(), "--bins".to_owned()]);

    eprintln!(
        "gate mutate: cargo-mutants@{version} ({}) …",
        scope_word(scope)
    );
    let output = exec(&argv, &dir, &[])?;
    let code = output.status.code().unwrap_or(-1);
    // Observed 27.1.0: 0 = all caught, 2 = missed, 3 = timeouts,
    // 4 = unviable; anything else is a tool failure.
    if !matches!(code, 0 | 2 | 3 | 4) {
        return Err(GateError::ToolFailed {
            tool: "cargo-mutants".to_owned(),
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

    // A diff touching no mutable code (comments, docs) yields zero
    // mutants — observed live: cargo-mutants then exits 0 WITHOUT writing
    // mutants.out. Exit 0 with no report is that case, not a tool failure.
    let out_dir = cache.join("mutants").join("mutants.out");
    if code == 0 && !out_dir.join("outcomes.json").is_file() {
        return Ok(StackRun::skipped(
            "cargo-mutants",
            "mutate[rust]: the diff touches no mutable code — zero mutants \
             (cargo-mutants wrote no report)"
                .to_owned(),
        ));
    }

    let outcomes_path = cache
        .join("mutants")
        .join("mutants.out")
        .join("outcomes.json");
    let text = std::fs::read_to_string(&outcomes_path).map_err(|source| GateError::Io {
        path: outcomes_path,
        source,
    })?;
    let (tally, findings) = parse_cargo_mutants_outcomes(&text, cwd)?;
    Ok(StackRun {
        tool: "cargo-mutants",
        tally,
        findings,
        note: None,
    })
}

/// Hermetic cargo-mutants: `cargo install` into
/// `~/.craftsman/tools/cargo-mutants@<version>/` on first use.
fn resolve_cargo_mutants(version: &str) -> Result<PathBuf, GateError> {
    let dir = tools::tools_dir()?.join(format!("cargo-mutants@{version}"));
    let bin = dir.join("bin").join("cargo-mutants");
    if bin.is_file() {
        return Ok(bin);
    }
    eprintln!("tool cargo-mutants@{version}: installing via cargo install (first use) …");
    let argv: Vec<String> = [
        "cargo",
        "install",
        "cargo-mutants",
        "--version",
        version,
        "--root",
        &dir.to_string_lossy(),
        "--locked",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let output = exec(&argv, Path::new("."), &[])?;
    if !output.status.success() || !bin.is_file() {
        return Err(GateError::ToolFailed {
            tool: "cargo install cargo-mutants".to_owned(),
            code: output
                .status
                .code()
                .map_or_else(|| "signal".to_owned(), |c| c.to_string()),
            output: tail(&String::from_utf8_lossy(&output.stderr), 30),
        });
    }
    Ok(bin)
}

/// The diff against `HEAD` for one stack, path-relative to its cwd so the
/// tool (running there) can match files. Untracked new files are not in
/// the diff — they carry no baseline behavior to regress (documented).
fn stack_diff(root: &Path, cwd: Option<&str>) -> Result<String, GateError> {
    cwd.map_or_else(
        || super::super::git(root, &["diff", "HEAD"]),
        |c| {
            let relative = format!("--relative={c}");
            super::super::git(root, &["diff", "HEAD", &relative, "--", c])
        },
    )
}

/// Parse `mutants.out/outcomes.json` (schema observed live against
/// cargo-mutants 27.1.0: top-level `caught`/`missed`/`timeout`/`unviable`
/// totals; `outcomes[].scenario.Mutant.{file, span.start.line, name}` with
/// `summary == "MissedMutant"` for survivors).
fn parse_cargo_mutants_outcomes(
    text: &str,
    cwd: Option<&str>,
) -> Result<(Tally, Vec<Finding>), GateError> {
    let doc: Value = serde_json::from_str(text).map_err(|e| GateError::Parse {
        tool: "cargo-mutants",
        detail: format!("invalid outcomes.json: {e}"),
    })?;
    let count = |key: &str| {
        doc[key]
            .as_u64()
            .map(usize::try_from)
            .and_then(Result::ok)
            .ok_or_else(|| GateError::Parse {
                tool: "cargo-mutants",
                detail: format!("outcomes.json lacks the `{key}` total"),
            })
    };
    let tally = Tally {
        caught: count("caught")?,
        missed: count("missed")?,
        timeout: count("timeout")?,
        unviable: count("unviable")?,
    };
    let mut findings = Vec::new();
    for outcome in doc["outcomes"].as_array().unwrap_or(&Vec::new()) {
        if outcome["summary"] != "MissedMutant" {
            continue;
        }
        let mutant = &outcome["scenario"]["Mutant"];
        let file = mutant["file"].as_str().unwrap_or_default();
        let file = cwd.map_or_else(|| file.to_owned(), |c| format!("{c}/{file}"));
        findings.push(Finding {
            gate: "mutate",
            tool: "cargo-mutants",
            rule: "survived-mutant".to_owned(),
            file,
            line: mutant["span"]["start"]["line"].as_u64(),
            message: mutant["name"]
                .as_str()
                .unwrap_or("mutant survived the test suite")
                .to_owned(),
            severity: Severity::Medium,
        });
    }
    Ok((tally, findings))
}

// ------------------------------------------------------------------- python

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_mutants_outcomes_parse_totals_and_survivors() {
        // Trimmed from a live cargo-mutants 27.1.0 run (scratch crate).
        let json = r#"{
            "outcomes": [
                {"scenario": "Baseline", "summary": "Success"},
                {"scenario": {"Mutant": {"name": "src/lib.rs:20:5: replace double -> i32 with 0",
                    "file": "src/lib.rs", "span": {"start": {"line": 20, "column": 5}}}},
                 "summary": "CaughtMutant"},
                {"scenario": {"Mutant": {"name": "src/lib.rs:20:7: replace * with + in double",
                    "file": "src/lib.rs", "span": {"start": {"line": 20, "column": 7}}}},
                 "summary": "MissedMutant"}
            ],
            "total_mutants": 5, "missed": 1, "caught": 4, "timeout": 0, "unviable": 0
        }"#;
        let (tally, findings) = parse_cargo_mutants_outcomes(json, Some("cli")).expect("parses");
        assert_eq!(tally.caught, 4);
        assert_eq!(tally.missed, 1);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "cli/src/lib.rs");
        assert_eq!(findings[0].line, Some(20));
        assert_eq!(findings[0].rule, "survived-mutant");
        assert!(findings[0].message.contains("replace * with +"));

        let err = parse_cargo_mutants_outcomes("{}", None).expect_err("totals required");
        assert!(matches!(err, GateError::Parse { .. }), "{err}");
    }
}

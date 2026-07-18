//! cucumber-js adapter (typescript stack), driven through bun.
//!
//! Invocation: `bunx cucumber-js --format message:<artifacts>/ts-msgs.ndjson
//! --format json:<artifacts>/ts.json` in the stack's project directory (bun
//! owns the environment; committed `bun.lock` pins the toolchain — repo
//! rule: bun, never npm/npx). Observed with bun 1.3.14 + cucumber-js 13.1.1:
//! behavior matches the ADR-002 npm facts — exit 1 on failure with both
//! artifacts written, and an unmatched `--name` exits 0 printing
//! `0 scenarios` with a well-formed empty artifact, so craftsman counts
//! scenarios itself (zero → dispatcher exit 4, never silent success).
//!
//! Selection: `--name` takes a regex matched against scenario names; one
//! anchored, regex-escaped `--name ^…$` per requested scenario (repeated
//! flags OR together).
//!
//! Ingestion: Cucumber Messages NDJSON is primary (richest — explicit
//! UNDEFINED/AMBIGUOUS, per-step results); the cucumber-json artifact is
//! the fallback when the NDJSON is missing or unreadable.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{AdapterError, tail};
use crate::verify::normalize::{
    CucumberJsonDialect, ScenarioResult, parse_cucumber_json, parse_messages_ndjson,
};

/// Run cucumber-js, optionally filtered to exact scenario names, and
/// normalize its Messages NDJSON (or cucumber-json fallback) output.
///
/// # Errors
/// [`AdapterError`] on spawn failure, a failing runner that produced no
/// artifacts, or artifacts neither parser can read.
pub fn run(
    project_dir: &Path,
    artifacts_dir: &Path,
    scenario_names: Option<&[String]>,
) -> Result<Vec<ScenarioResult>, AdapterError> {
    let (ndjson_path, json_path) = fresh_artifact_paths(artifacts_dir)?;

    let mut cmd = Command::new("bunx");
    cmd.arg("cucumber-js")
        .arg(format!("--format=message:{}", ndjson_path.display()))
        .arg(format!("--format=json:{}", json_path.display()));
    if let Some(names) = scenario_names {
        for name in names {
            cmd.arg("--name").arg(exact_name_regex(name));
        }
    }
    let command_line = "bunx cucumber-js".to_owned();

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

    if let Some(results) = ingest_artifacts(&ndjson_path, &json_path)? {
        return Ok(results);
    }

    if output.status.success() {
        return Err(AdapterError::NoResults {
            path: ndjson_path,
            hint: "cucumber-js ran green but wrote no artifacts — is \
                   @cucumber/cucumber installed (bun install)?"
                .to_owned(),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(AdapterError::RunnerFailed {
        command: command_line,
        code: output
            .status
            .code()
            .map_or_else(|| "signal".to_owned(), |c| c.to_string()),
        output_tail: tail(&stdout, 20),
    })
}

/// Ensure the artifacts dir exists and both artifact slots are fresh
/// (stale files removed); returns `(ndjson, json)` paths.
fn fresh_artifact_paths(artifacts_dir: &Path) -> Result<(PathBuf, PathBuf), AdapterError> {
    std::fs::create_dir_all(artifacts_dir).map_err(|source| AdapterError::ResultsPath {
        path: artifacts_dir.to_path_buf(),
        source,
    })?;
    let ndjson_path = artifacts_dir.join("ts-msgs.ndjson");
    let json_path = artifacts_dir.join("ts.json");
    for path in [&ndjson_path, &json_path] {
        if path.exists() {
            std::fs::remove_file(path).map_err(|source| AdapterError::ResultsPath {
                path: path.clone(),
                source,
            })?;
        }
    }
    Ok((ndjson_path, json_path))
}

/// Read the run's artifacts: NDJSON primary, cucumber-json fallback
/// (module docs). `None` = neither artifact exists — the caller judges
/// the runner's exit status. The artifacts are the sole verdict source:
/// cucumber-js exits 1 on red scenarios after writing them.
fn ingest_artifacts(
    ndjson_path: &Path,
    json_path: &Path,
) -> Result<Option<Vec<ScenarioResult>>, AdapterError> {
    if ndjson_path.is_file() {
        let text =
            std::fs::read_to_string(ndjson_path).map_err(|source| AdapterError::ReadResults {
                path: ndjson_path.to_path_buf(),
                source,
            })?;
        match parse_messages_ndjson(&text) {
            Ok(results) => return Ok(Some(results)),
            Err(err) => {
                eprintln!(
                    "cucumber-js: messages NDJSON unreadable ({err}) — \
                     falling back to the cucumber-json artifact"
                );
            }
        }
    }
    if json_path.is_file() {
        let text =
            std::fs::read_to_string(json_path).map_err(|source| AdapterError::ReadResults {
                path: json_path.to_path_buf(),
                source,
            })?;
        return Ok(Some(parse_cucumber_json(
            &text,
            CucumberJsonDialect::Generic,
        )?));
    }
    Ok(None)
}

/// Anchored, escaped regex matching exactly one scenario name — cucumber-js
/// `--name` is a JS regex, so every ECMA-262 metacharacter is escaped.
fn exact_name_regex(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 2);
    out.push('^');
    for c in name.chars() {
        if "\\^$.|?*+()[]{}/".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('$');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_regex_is_anchored_and_escaped() {
        assert_eq!(exact_name_regex("Plain name"), "^Plain name$");
        assert_eq!(
            exact_name_regex("Costs $4.99 (really)?"),
            r"^Costs \$4\.99 \(really\)\?$"
        );
    }
}

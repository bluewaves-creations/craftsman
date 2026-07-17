//! cucumber-rs adapter (rust stack).
//!
//! Convention (ADR-003): the project's cucumber-rs harness is a cargo
//! integration-test target (`cargo test --test <runner-target>`, default
//! `spec`) that honors the `CRAFTSMAN_JSON` environment variable by writing
//! cucumber-json (`writer::Json`, cargo feature `output-json`) to that path.
//! The JSON artifact is the sole verdict source — cucumber-rs exits 0 even
//! on failing scenarios when a file writer is used, and a `--name` filter
//! matching nothing also exits 0 (ADR-002); craftsman counts scenarios
//! itself.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use thiserror::Error;

use crate::verify::normalize::{
    CucumberJsonDialect, NormalizeError, ScenarioResult, parse_cucumber_json,
};

/// Environment variable through which the adapter hands the harness the
/// cucumber-json output path.
pub const RESULTS_ENV: &str = "CRAFTSMAN_JSON";

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("failed to spawn `{command}` in {dir}")]
    Spawn {
        command: String,
        dir: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not prepare results path {path}")]
    ResultsPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("`{command}` failed (exit {code}) without writing results:\n{output_tail}")]
    RunnerFailed {
        command: String,
        code: String,
        output_tail: String,
    },
    #[error(
        "harness wrote no results to {path} — the `--test {target}` target must \
         write cucumber-json to the path in ${RESULTS_ENV} (ADR-003 convention)"
    )]
    NoResults { path: PathBuf, target: String },
    #[error("cannot read results {path}")]
    ReadResults {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Normalize(#[from] NormalizeError),
}

/// Run the project's cucumber-rs harness, optionally filtered to exact
/// scenario names, and normalize its cucumber-json output.
///
/// Runner stdout is forwarded to our stderr (stdout belongs to `--json`);
/// runner stderr (cargo build progress) is inherited.
///
/// # Errors
/// [`AdapterError`] on spawn failure, a failing runner that produced no
/// results, a missing results file, or unparseable results.
pub fn run(
    project_dir: &Path,
    runner_target: &str,
    scenario_names: Option<&[String]>,
) -> Result<Vec<ScenarioResult>, AdapterError> {
    let results_dir = project_dir.join("target").join("craftsman");
    std::fs::create_dir_all(&results_dir).map_err(|source| AdapterError::ResultsPath {
        path: results_dir.clone(),
        source,
    })?;
    let results_path = results_dir.join(format!("verify-{runner_target}.json"));
    if results_path.exists() {
        std::fs::remove_file(&results_path).map_err(|source| AdapterError::ResultsPath {
            path: results_path.clone(),
            source,
        })?;
    }

    let mut cmd = Command::new("cargo");
    cmd.arg("test").arg("--test").arg(runner_target);
    if let Some(names) = scenario_names {
        cmd.arg("--").arg("--name").arg(names_pattern(names));
    }
    let command_line = format!("cargo test --test {runner_target}");

    let output = cmd
        .current_dir(project_dir)
        .env(RESULTS_ENV, &results_path)
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

    if !results_path.is_file() {
        if output.status.success() {
            return Err(AdapterError::NoResults {
                path: results_path,
                target: runner_target.to_owned(),
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(AdapterError::RunnerFailed {
            command: command_line,
            code: output
                .status
                .code()
                .map_or_else(|| "signal".to_owned(), |c| c.to_string()),
            output_tail: tail(&stdout, 20),
        });
    }

    let text =
        std::fs::read_to_string(&results_path).map_err(|source| AdapterError::ReadResults {
            path: results_path.clone(),
            source,
        })?;
    // The JSON artifact is the verdict source even when the harness process
    // failed (run_and_exit panics on red scenarios after writing the file).
    Ok(parse_cucumber_json(&text, CucumberJsonDialect::CucumberRs)?)
}

/// Anchored alternation of regex-escaped names for cucumber-rs `--name`
/// (a regex matched against the scenario name).
fn names_pattern(names: &[String]) -> String {
    let escaped: Vec<String> = names.iter().map(|n| regex_escape(n)).collect();
    format!("^({})$", escaped.join("|"))
}

/// Escape the metacharacter set of the `regex` crate (what cucumber-rs
/// matches `--name` with) — mirrors `regex::escape` to avoid pulling the
/// crate in for one function.
fn regex_escape(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_pattern_is_anchored_and_escaped() {
        let names = vec!["Plain name".to_owned(), "Costs $4.99 (really)".to_owned()];
        assert_eq!(
            names_pattern(&names),
            r"^(Plain name|Costs \$4\.99 \(really\))$"
        );
    }
}

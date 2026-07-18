//! pytest-bdd adapter (python stack).
//!
//! Invocation: `uv run pytest <tests-dir> --cucumberjson=<artifacts>/py.json
//! --junitxml=<artifacts>/py-junit.xml` in the stack's project directory
//! (uv owns the environment; committed `uv.lock` pins the toolchain).
//!
//! ADR-002 (all observed, not quoted): pytest-bdd's cucumber-json silently
//! OMITS an undefined step and everything after it, so an unimplemented
//! scenario aggregates to PASSED there; only the `JUnit` artifact carries the
//! truth (`StepDefinitionNotFoundError` → Undefined). Neither artifact alone
//! is sufficient — json lies about undefined, junit lies about names (its
//! testcase names are mangled pytest ids). This adapter therefore runs with
//! BOTH writers and merges: names/steps/durations from cucumber-json,
//! severity max'd with the `JUnit` verdict matched via the derived test id.
//!
//! Selection: scenario names map to pytest `-k` expressions over the mangled
//! test ids (`test_<name>` per pytest-bdd's `make_python_name`, observed on
//! 8.1.0: spaces → `_`, other non-word chars dropped, leading digits plus
//! their underscores stripped, lowercased). `-k` matches substrings, so the
//! filtered run may over-select — results are post-filtered by exact
//! scenario name. If the derived ids under-selected (a requested scenario
//! missing from the filtered results — mangling drift), the adapter reruns
//! unfiltered and post-filters: conservative correctness over cleverness.
//! An empty `-k` match exits 5 (error-shaped, ADR-002) → empty results here,
//! which the dispatcher maps to craftsman exit 4.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};

use super::{AdapterError, tail};
use crate::verify::normalize::{
    CucumberJsonDialect, JunitDialect, ScenarioResult, Status, parse_cucumber_json, parse_junit,
};

/// pytest exit code for "no tests collected" (empty `-k` match).
const PYTEST_EXIT_NO_TESTS: i32 = 5;

/// One pytest-bdd run's outcome.
#[derive(Debug)]
pub struct PytestRun {
    pub results: Vec<ScenarioResult>,
    /// `coverage json --show-contexts` document text, when coverage capture
    /// was requested and every step of it succeeded. Never fails the run —
    /// the impact map is an optimization, not a verdict.
    pub coverage_json: Option<String>,
}

/// Run pytest-bdd, optionally filtered to exact scenario names, and merge
/// its cucumber-json + `JUnit` artifacts into normalized results.
///
/// # Errors
/// [`AdapterError`] on spawn failure, a failing runner that produced no
/// artifacts, or unparseable artifacts.
pub fn run(
    project_dir: &Path,
    artifacts_dir: &Path,
    tests_dir: &str,
    scenario_names: Option<&[String]>,
    capture_coverage: bool,
) -> Result<PytestRun, AdapterError> {
    let mut results = run_once(
        project_dir,
        artifacts_dir,
        tests_dir,
        scenario_names,
        capture_coverage,
    )?;

    if let Some(names) = scenario_names {
        results.results.retain(|r| names.contains(&r.scenario));
        if names
            .iter()
            .any(|n| !results.results.iter().any(|r| &r.scenario == n))
            && !results.results.is_empty()
        {
            // The -k expression under-selected (see module docs): rerun
            // unfiltered and post-filter — never silently narrower.
            eprintln!(
                "pytest-bdd: derived test ids missed a requested scenario — \
                 rerunning unfiltered and filtering results"
            );
            results = run_once(
                project_dir,
                artifacts_dir,
                tests_dir,
                None,
                capture_coverage,
            )?;
            results.results.retain(|r| names.contains(&r.scenario));
        }
    }
    Ok(results)
}

fn run_once(
    project_dir: &Path,
    artifacts_dir: &Path,
    tests_dir: &str,
    scenario_names: Option<&[String]>,
    capture_coverage: bool,
) -> Result<PytestRun, AdapterError> {
    std::fs::create_dir_all(artifacts_dir).map_err(|source| AdapterError::ResultsPath {
        path: artifacts_dir.to_path_buf(),
        source,
    })?;
    let json_path = artifacts_dir.join("py.json");
    let junit_path = artifacts_dir.join("py-junit.xml");
    for path in [&json_path, &junit_path] {
        if path.exists() {
            std::fs::remove_file(path).map_err(|source| AdapterError::ResultsPath {
                path: path.clone(),
                source,
            })?;
        }
    }

    let with_coverage = capture_coverage && pytest_cov_available(project_dir);
    let mut cmd = Command::new("uv");
    cmd.arg("run")
        .arg("pytest")
        .arg(tests_dir)
        .arg(format!("--cucumberjson={}", json_path.display()))
        .arg(format!("--junitxml={}", junit_path.display()));
    if with_coverage {
        // Scope coverage to the project directory; contexts key each line
        // by the running test (pytest-cov `<nodeid>|<phase>`).
        cmd.arg("--cov=.").arg("--cov-context=test");
    }
    if let Some(names) = scenario_names {
        cmd.arg("-k").arg(k_expression(names));
    }
    let command_line = format!("uv run pytest {tests_dir}");

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

    if output.status.code() == Some(PYTEST_EXIT_NO_TESTS) {
        // Empty -k match is an error-shaped exit 5, not 0/1 (ADR-002).
        // Zero scenarios → the dispatcher's exit-4 path.
        return Ok(PytestRun {
            results: Vec::new(),
            coverage_json: None,
        });
    }
    if !json_path.is_file() || !junit_path.is_file() {
        if output.status.success() {
            return Err(AdapterError::NoResults {
                path: json_path,
                hint: "pytest ran green but wrote no cucumber-json/JUnit artifacts — \
                       is pytest-bdd installed in the project environment?"
                    .to_owned(),
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

    let read = |path: &Path| {
        std::fs::read_to_string(path).map_err(|source| AdapterError::ReadResults {
            path: path.to_path_buf(),
            source,
        })
    };
    let json_results = parse_cucumber_json(&read(&json_path)?, CucumberJsonDialect::Generic)?;
    let junit_results = parse_junit(&read(&junit_path)?, JunitDialect::PytestBdd)?;
    let results = merge(json_results, &junit_results);

    let coverage_json = with_coverage.then(|| capture_coverage_json(project_dir, artifacts_dir));
    Ok(PytestRun {
        results,
        coverage_json: coverage_json.flatten(),
    })
}

/// Merge cucumber-json rows (authoritative names/durations) with the `JUnit`
/// verdicts (authoritative severity — the only artifact that sees UNDEFINED).
///
/// Matched by derived test id; merged status is the severity max, so a
/// scenario whose json says Passed but whose junit says
/// `StepDefinitionNotFoundError` comes out Undefined. `JUnit` rows with no
/// json counterpart are kept as-is (mangled name — better honest than
/// absent; that shape only appears when cucumber-json dropped a scenario
/// entirely).
fn merge(json_rows: Vec<ScenarioResult>, junit_rows: &[ScenarioResult]) -> Vec<ScenarioResult> {
    let mut by_test_id: HashMap<&str, &ScenarioResult> = junit_rows
        .iter()
        .map(|r| (r.scenario.as_str(), r))
        .collect();

    let mut out: Vec<ScenarioResult> = json_rows
        .into_iter()
        .map(|mut row| {
            if let Some(junit) = by_test_id.remove(python_test_id(&row.scenario).as_str()) {
                if junit.status > row.status {
                    row.status = junit.status;
                }
                if row.failure.is_none() && row.status != Status::Passed {
                    row.failure.clone_from(&junit.failure);
                }
            }
            row
        })
        .collect();
    out.extend(by_test_id.into_values().cloned());
    out
}

/// The pytest test id pytest-bdd generates for a scenario name — observed
/// behavior of `make_python_name` in 8.1.0 (see module docs), verified
/// against the committed fixture artifacts.
pub(crate) fn python_test_id(scenario: &str) -> String {
    let cleaned: String = scenario
        .chars()
        .map(|c| if c == ' ' { '_' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    let mut rest = cleaned.as_str();
    if rest.starts_with(|c: char| c.is_ascii_digit()) {
        rest = rest
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .trim_start_matches('_');
    }
    format!("test_{}", rest.to_lowercase())
}

/// `-k` expression selecting the derived test ids (OR of substring matches).
fn k_expression(names: &[String]) -> String {
    let ids: Vec<String> = names.iter().map(|n| python_test_id(n)).collect();
    ids.join(" or ")
}

/// Whether pytest-cov is importable in the project environment (cheap probe
/// so a project without it never sees unknown `--cov` flags).
fn pytest_cov_available(project_dir: &Path) -> bool {
    Command::new("uv")
        .args(["run", "python", "-c", "import pytest_cov"])
        .current_dir(project_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// `uv run coverage json --show-contexts` after a `--cov` run. Any failure
/// returns `None` with a stderr note — coverage never fails the verdict.
fn capture_coverage_json(project_dir: &Path, artifacts_dir: &Path) -> Option<String> {
    let out_path = artifacts_dir.join("py-coverage.json");
    let result = Command::new("uv")
        .args(["run", "coverage", "json", "--show-contexts", "-o"])
        .arg(&out_path)
        .current_dir(project_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match result {
        Ok(status) if status.success() => std::fs::read_to_string(&out_path).ok(),
        _ => {
            eprintln!("pytest-bdd: coverage export failed — impact map not updated for python");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_mangling_matches_pytest_bdd() {
        // Ground truth: the ids observed in the committed fixture artifacts
        // (tests/fixtures/py.json `elements[].id`).
        assert_eq!(
            python_test_id("Add an item to the list"),
            "test_add_an_item_to_the_list"
        );
        assert_eq!(
            python_test_id("Adding one item yields two items"),
            "test_adding_one_item_yields_two_items"
        );
        // Punctuation is dropped (not underscored), leading digits stripped.
        assert_eq!(
            python_test_id("Costs $4.99 (really)"),
            "test_costs_499_really"
        );
        assert_eq!(python_test_id("2nd try wins"), "test_nd_try_wins");
    }

    /// THE ADR-002 merge proof, against the real S2 artifacts: pytest-bdd's
    /// cucumber-json reports the unimplemented scenario as Passed (the
    /// undefined step is silently omitted); only the `JUnit` artifact carries
    /// `StepDefinitionNotFoundError`. The merge must come out Undefined.
    #[test]
    fn merge_recovers_undefined_from_junit() {
        let fixture = |name: &str| {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");
            std::fs::read_to_string(format!("{path}{name}"))
                .unwrap_or_else(|e| panic!("{name}: {e}"))
        };
        let json = parse_cucumber_json(&fixture("py.json"), CucumberJsonDialect::Generic)
            .expect("fixture json parses");
        // json alone falls into the trap: the undefined scenario looks green.
        assert_eq!(json[2].scenario, "Remove an item from the list");
        assert_eq!(json[2].status, Status::Passed);

        let junit = parse_junit(&fixture("py-junit.xml"), JunitDialect::PytestBdd)
            .expect("fixture junit parses");
        let merged = merge(json, &junit);

        let statuses: Vec<(&str, Status)> = merged
            .iter()
            .map(|r| (r.scenario.as_str(), r.status))
            .collect();
        assert_eq!(
            statuses,
            vec![
                ("Add an item to the list", Status::Passed),
                ("Adding one item yields two items", Status::Failed),
                ("Remove an item from the list", Status::Undefined),
            ]
        );
        // The undefined row borrows junit's failure detail (json has none).
        assert!(
            merged[2]
                .failure
                .as_deref()
                .expect("failure detail")
                .contains("StepDefinitionNotFoundError")
        );
        // Names stay the clean cucumber-json ones, never the mangled ids.
        assert!(merged.iter().all(|r| !r.scenario.starts_with("test_")));
    }

    #[test]
    fn k_expression_is_an_or_of_test_ids() {
        let names = vec!["First thing".to_owned(), "Second thing".to_owned()];
        assert_eq!(
            k_expression(&names),
            "test_first_thing or test_second_thing"
        );
    }
}

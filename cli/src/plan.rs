//! Plan parsing — the batch → scenario mapping.
//!
//! Batching lives in the plan, never in Gherkin tags (design decision #1):
//! each `## Batch N` section carries a `Scenarios:` list of scenario names,
//! one `- ` bullet per name. `craftsman verify --batch N` resolves names
//! here and synthesizes the runner's native filter; `craftsman plan lint`
//! keeps the whole mapping honest against the spec inventory.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::spec::Finding;

/// Errors resolving a batch from the plan. Exit code 3 territory.
#[derive(Debug, Error)]
pub enum PlanError {
    #[error("failed to read plan {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("plan {path} has no `## Batch {batch}` section")]
    BatchMissing { path: PathBuf, batch: u32 },
    #[error(
        "plan {path} section `## Batch {batch}` has no `Scenarios:` list — \
         add one (`Scenarios:` followed by `- <scenario name>` bullets)"
    )]
    ScenariosMissing { path: PathBuf, batch: u32 },
}

/// One `## Batch N` section of the plan with its (possibly absent)
/// `Scenarios:` list.
#[derive(Debug, Clone)]
pub struct PlanBatch {
    pub number: u32,
    /// 1-based line of the `## Batch N` heading.
    pub line: usize,
    /// `(line, scenario name)` bullets under `Scenarios:`; empty when the
    /// section carries no list (batches not yet detailed — legal).
    pub scenarios: Vec<(usize, String)>,
}

/// Read the plan file and parse every batch section.
///
/// # Errors
/// [`PlanError::Read`] when the plan cannot be read.
pub fn parse_plan(path: &Path) -> Result<Vec<PlanBatch>, PlanError> {
    let text = std::fs::read_to_string(path).map_err(|source| PlanError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(batches(&text))
}

/// Resolve the scenario names of `## Batch N` in the plan file.
///
/// # Errors
/// [`PlanError::Read`] when the plan cannot be read; [`PlanError::BatchMissing`]
/// when no `## Batch N` heading exists; [`PlanError::ScenariosMissing`] when
/// the section has no non-empty `Scenarios:` bullet list.
pub fn batch_scenarios(path: &Path, batch: u32) -> Result<Vec<String>, PlanError> {
    let section = parse_plan(path)?
        .into_iter()
        .find(|b| b.number == batch)
        .ok_or_else(|| PlanError::BatchMissing {
            path: path.to_path_buf(),
            batch,
        })?;
    if section.scenarios.is_empty() {
        return Err(PlanError::ScenariosMissing {
            path: path.to_path_buf(),
            batch,
        });
    }
    Ok(section
        .scenarios
        .into_iter()
        .map(|(_, name)| name)
        .collect())
}

/// Every `## Batch N` section with its `Scenarios:` bullets. Any other
/// `##`-prefixed heading closes the current section.
#[must_use]
pub fn batches(text: &str) -> Vec<PlanBatch> {
    let mut out: Vec<PlanBatch> = Vec::new();
    let mut in_batch = false;
    let mut in_list = false;
    for (idx, line) in text.lines().enumerate() {
        let lineno = idx + 1;
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("## ") {
            in_list = false;
            in_batch = batch_number(rest).is_some_and(|number| {
                out.push(PlanBatch {
                    number,
                    line: lineno,
                    scenarios: Vec::new(),
                });
                true
            });
        } else if in_batch {
            if in_list {
                if let Some(bullet) = trimmed.strip_prefix("- ") {
                    let name = bullet.trim().trim_matches('`').trim_matches('"').trim();
                    // `in_batch` is only true right after a push above, so
                    // `out.last_mut()` is always `Some` here.
                    if let (false, Some(batch)) = (name.is_empty(), out.last_mut()) {
                        batch.scenarios.push((lineno, name.to_owned()));
                    }
                } else if !trimmed.is_empty() {
                    in_list = false; // list ended
                }
            } else if trimmed.starts_with("Scenarios:") {
                in_list = true;
            }
        }
    }
    out
}

/// `Batch N` optionally followed by punctuation/title (`## Batch 2 — ...`),
/// but never `## Batch 21` when the digits are `21`, not `2`.
fn batch_number(rest: &str) -> Option<u32> {
    let after = rest.strip_prefix("Batch ")?;
    let digits: &str = &after[..after
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit())
        .map_or(after.len(), |(i, _)| i)];
    digits.parse().ok()
}

/// Lint the plan's batch → scenario mapping against the spec inventory.
///
/// Errors: a listed scenario missing from the spec; a scenario assigned to
/// two batches. Warning: a spec scenario assigned to no batch (coverage gap
/// — reported, not fatal). Findings carry plan-file line numbers (0 for
/// unassigned-scenario findings, which have no plan line).
#[must_use]
pub fn lint(plan_batches: &[PlanBatch], spec_scenarios: &[String]) -> Vec<Finding> {
    let known: HashSet<&str> = spec_scenarios.iter().map(String::as_str).collect();
    let mut assigned: HashMap<&str, u32> = HashMap::new();
    let mut findings = Vec::new();

    for batch in plan_batches {
        for (line, name) in &batch.scenarios {
            if !known.contains(name.as_str()) {
                findings.push(Finding::error(
                    "unknown-scenario",
                    *line,
                    format!(
                        "batch {} lists scenario {name:?} which is not in the spec — \
                         plan drift; fix the plan (only the human changes the spec)",
                        batch.number
                    ),
                ));
            }
            if let Some(&first) = assigned.get(name.as_str()) {
                findings.push(Finding::error(
                    "duplicate-assignment",
                    *line,
                    format!(
                        "scenario {name:?} is already assigned to batch {first} — \
                         a scenario belongs to at most one batch"
                    ),
                ));
            } else {
                assigned.insert(name, batch.number);
            }
        }
    }

    for name in spec_scenarios {
        if !assigned.contains_key(name.as_str()) {
            findings.push(Finding::warning(
                "unassigned-scenario",
                0,
                format!("spec scenario {name:?} is not assigned to any plan batch"),
            ));
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::Severity;

    const PLAN: &str = "\
# Plan

## Batch 1 — groundwork

Scenarios:
- First behavior
- `Second behavior`

Notes after the list.

## Batch 2: follow-up

Some prose.

Scenarios:
- Third behavior

## Batch 21

No scenarios list here.
";

    fn from_text(text: &str, batch: u32) -> Result<Vec<String>, PlanError> {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("PLAN.md");
        std::fs::write(&path, text).expect("write plan");
        batch_scenarios(&path, batch)
    }

    #[test]
    fn resolves_bullets_under_scenarios() {
        assert_eq!(
            from_text(PLAN, 1).expect("batch 1 exists"),
            vec!["First behavior".to_owned(), "Second behavior".to_owned()]
        );
        assert_eq!(
            from_text(PLAN, 2).expect("batch 2 exists"),
            vec!["Third behavior".to_owned()]
        );
    }

    #[test]
    fn batch_number_matches_exactly() {
        // Batch 21 exists but has no Scenarios list; batch 2 must not match it.
        let err = from_text(PLAN, 21).expect_err("batch 21 has no list");
        assert!(
            matches!(err, PlanError::ScenariosMissing { batch: 21, .. }),
            "{err}"
        );
    }

    #[test]
    fn missing_batch_is_an_error() {
        let err = from_text(PLAN, 7).expect_err("no batch 7");
        assert!(
            matches!(err, PlanError::BatchMissing { batch: 7, .. }),
            "{err}"
        );
    }

    #[test]
    fn missing_plan_file_is_an_error() {
        let err = batch_scenarios(Path::new("/nonexistent/PLAN.md"), 1).expect_err("no file");
        assert!(matches!(err, PlanError::Read { .. }), "{err}");
    }

    #[test]
    fn batches_parses_every_section_with_lines() {
        let all = batches(PLAN);
        let numbers: Vec<u32> = all.iter().map(|b| b.number).collect();
        assert_eq!(numbers, vec![1, 2, 21]);
        assert_eq!(all[0].scenarios.len(), 2);
        assert_eq!(all[0].scenarios[0], (6, "First behavior".to_owned()));
        assert!(all[2].scenarios.is_empty());
    }

    #[test]
    fn scenario_bullets_outside_a_batch_are_ignored() {
        let text = "## Not a batch\n\nScenarios:\n- Stray behavior\n";
        assert!(batches(text).is_empty());
    }

    fn rules(findings: &[Finding], severity: Severity) -> Vec<&'static str> {
        findings
            .iter()
            .filter(|f| f.severity == severity)
            .map(|f| f.rule)
            .collect()
    }

    fn spec_names(names: &[&str]) -> Vec<String> {
        names.iter().map(|&n| n.to_owned()).collect()
    }

    #[test]
    fn lint_accepts_full_coverage() {
        let findings = lint(
            &batches(PLAN),
            &spec_names(&["First behavior", "Second behavior", "Third behavior"]),
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn lint_flags_scenarios_missing_from_the_spec() {
        let findings = lint(&batches(PLAN), &spec_names(&["First behavior"]));
        assert_eq!(
            rules(&findings, Severity::Error),
            vec!["unknown-scenario", "unknown-scenario"]
        );
        assert!(findings[0].message.contains("Second behavior"));
    }

    #[test]
    fn lint_flags_a_scenario_assigned_twice() {
        let plan =
            "## Batch 1\n\nScenarios:\n- Same thing\n\n## Batch 2\n\nScenarios:\n- Same thing\n";
        let findings = lint(&batches(plan), &spec_names(&["Same thing"]));
        assert_eq!(
            rules(&findings, Severity::Error),
            vec!["duplicate-assignment"]
        );
        assert!(findings[0].message.contains("batch 1"));
    }

    #[test]
    fn lint_reports_unassigned_spec_scenarios_as_warnings() {
        let findings = lint(
            &batches(PLAN),
            &spec_names(&[
                "First behavior",
                "Second behavior",
                "Third behavior",
                "Orphan behavior",
            ]),
        );
        assert_eq!(rules(&findings, Severity::Error), Vec::<&str>::new());
        assert_eq!(
            rules(&findings, Severity::Warning),
            vec!["unassigned-scenario"]
        );
        assert!(findings[0].message.contains("Orphan behavior"));
    }
}

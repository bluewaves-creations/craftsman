//! PLAN.md parsing — the batch → scenario mapping.
//!
//! Batching lives in the plan, never in Gherkin tags (design decision #1):
//! each `## Batch N` section carries a `Scenarios:` list of scenario names,
//! one `- ` bullet per name. `craftsman verify --batch N` resolves names
//! here and synthesizes the runner's native filter.

use std::path::{Path, PathBuf};

use thiserror::Error;

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

/// Resolve the scenario names of `## Batch N` in the plan file.
///
/// # Errors
/// [`PlanError::Read`] when the plan cannot be read; [`PlanError::BatchMissing`]
/// when no `## Batch N` heading exists; [`PlanError::ScenariosMissing`] when
/// the section has no non-empty `Scenarios:` bullet list.
pub fn batch_scenarios(path: &Path, batch: u32) -> Result<Vec<String>, PlanError> {
    let text = std::fs::read_to_string(path).map_err(|source| PlanError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let section = batch_section(&text, batch).ok_or_else(|| PlanError::BatchMissing {
        path: path.to_path_buf(),
        batch,
    })?;
    let names = scenarios_list(section);
    if names.is_empty() {
        return Err(PlanError::ScenariosMissing {
            path: path.to_path_buf(),
            batch,
        });
    }
    Ok(names)
}

/// The lines of the `## Batch N` section (heading exclusive, next `## `
/// heading exclusive), or `None` if the heading does not exist.
fn batch_section(text: &str, batch: u32) -> Option<&str> {
    let mut start = None;
    for (offset, line) in line_offsets(text) {
        let trimmed = line.trim_end();
        if let Some(rest) = trimmed.strip_prefix("## ") {
            if let Some(s) = start {
                return Some(&text[s..offset]);
            }
            if is_batch_heading(rest, batch) {
                start = Some(offset + line.len());
            }
        }
    }
    start.map(|s| &text[s..])
}

/// `Batch N` optionally followed by punctuation/title (`## Batch 2 — ...`),
/// but not `## Batch 21` when looking for batch 2.
fn is_batch_heading(rest: &str, batch: u32) -> bool {
    rest.strip_prefix("Batch ")
        .and_then(|r| r.strip_prefix(&batch.to_string()))
        .is_some_and(|after| !after.starts_with(|c: char| c.is_ascii_digit()))
}

fn line_offsets(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split_inclusive('\n').scan(0, |offset, line| {
        let start = *offset;
        *offset += line.len();
        Some((start, line))
    })
}

/// The `- ` bullets directly under the section's `Scenarios:` line.
fn scenarios_list(section: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_list = false;
    for line in section.lines() {
        let trimmed = line.trim();
        if in_list {
            if let Some(name) = trimmed.strip_prefix("- ") {
                let name = name.trim().trim_matches('`').trim_matches('"').trim();
                if !name.is_empty() {
                    names.push(name.to_owned());
                }
            } else if !trimmed.is_empty() {
                break; // list ended
            }
        } else if trimmed.starts_with("Scenarios:") {
            in_list = true;
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

//! SPEC.delta.md — the approved-but-unimplemented scenarios waiting for
//! their boundary merge. The delta keeps mid-batch verify runs meaningful:
//! the executed spec only ever holds scenarios whose meaning is live.

use std::path::{Path, PathBuf};

use gherkin::Feature;

use super::{Finding, SpecError, all_scenarios, lint, parse_spec};

/// The delta file sits next to the executed spec: approved-but-
/// unimplemented scenarios wait there until the boundary merge, so
/// mid-batch verify runs keep their meaning.
#[must_use]
pub fn delta_path(spec_path: &Path) -> PathBuf {
    spec_path.parent().map_or_else(
        || PathBuf::from("SPEC.delta.md"),
        |dir| dir.join("SPEC.delta.md"),
    )
}

/// The scenario names waiting in the delta next to `spec_path` — empty
/// when no delta exists or it does not parse (`spec lint --delta` owns
/// reporting a broken delta; consumers here only need the names).
#[must_use]
pub fn delta_scenario_names(spec_path: &Path) -> Vec<String> {
    parse_spec(&delta_path(spec_path)).map_or_else(
        |_| Vec::new(),
        |f| {
            super::inventory(&f)
                .into_iter()
                .map(|e| e.scenario)
                .collect()
        },
    )
}

/// Lint a delta feature: the full authoring rules plus name collisions
/// against the executed spec's scenario names.
#[must_use]
pub fn lint_delta(delta: &Feature, spec_names: &[String]) -> Vec<Finding> {
    let mut findings = lint(delta);
    for scenario in all_scenarios(delta) {
        if spec_names.iter().any(|n| n == &scenario.name) {
            findings.push(Finding::error(
                "delta-name-collision",
                scenario.position.line,
                format!(
                    "delta scenario {:?} collides with a scenario already in the \
                     executed spec — names are unique across spec and delta",
                    scenario.name
                ),
            ));
        }
    }
    findings
}

/// The scenario blocks of the delta text: everything from the first
/// `Scenario:` line, backed up over any tag lines directly above it.
fn scenario_blocks(delta_text: &str) -> Option<String> {
    let lines: Vec<&str> = delta_text.lines().collect();
    let mut start = lines
        .iter()
        .position(|l| l.trim_start().starts_with("Scenario:"))?;
    while start > 0 && lines[start - 1].trim_start().starts_with('@') {
        start -= 1;
    }
    Some(lines[start..].join("\n"))
}

/// Fold the delta's scenarios into the executed spec under a banner and
/// remove the delta file; returns how many scenarios moved.
///
/// The write is mediated (single-writer covers the merge) and never
/// commits — the repository head stays where it was. The caller lints
/// first; a delta with no scenarios is refused as a parse-level problem.
///
/// # Errors
/// [`SpecError::Read`] / [`SpecError::Parse`] on either file,
/// [`SpecError::Write`] when the merged spec cannot be written back.
pub fn merge_delta(spec_path: &Path, delta: &Path) -> Result<usize, SpecError> {
    let delta_feature = parse_spec(delta)?;
    let moved = all_scenarios(&delta_feature).count();
    let read = |path: &Path| {
        std::fs::read_to_string(path).map_err(|source| SpecError::Read {
            path: path.to_path_buf(),
            source,
        })
    };
    let delta_text = read(delta)?;
    let blocks = scenario_blocks(&delta_text).ok_or_else(|| SpecError::Parse {
        path: delta.to_path_buf(),
        message: "the delta holds no Scenario: blocks to merge".to_owned(),
    })?;
    let mut spec_text = read(spec_path)?;
    if !spec_text.ends_with('\n') {
        spec_text.push('\n');
    }
    let merged = format!(
        "{spec_text}\n  # ————— Merged from SPEC.delta.md (approved delta) —————\n\n{blocks}\n"
    );
    std::fs::write(spec_path, merged).map_err(|source| SpecError::Write {
        path: spec_path.to_path_buf(),
        source,
    })?;
    std::fs::remove_file(delta).map_err(|source| SpecError::Write {
        path: delta.to_path_buf(),
        source,
    })?;
    Ok(moved)
}

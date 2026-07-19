//! SPEC.md engine — scenario inventory and authoring lint.
//!
//! Built on the `gherkin` crate (the cucumber-rs parser; see AGENTS.md
//! Documentation Sources). The spec file is one Gherkin feature document,
//! human-owned.

use std::path::{Path, PathBuf};

use gherkin::{Feature, GherkinEnv};
use serde::Serialize;
use thiserror::Error;

mod delta;
pub use delta::{delta_path, delta_scenario_names, lint_delta, merge_delta};

/// Errors reading or parsing the spec.
///
/// Read/parse failures are exit code 3 for inventory consumers; `spec lint`
/// instead reports a parse failure as a lint error finding (a spec that does
/// not parse cannot be verified).
#[derive(Debug, Error)]
pub enum SpecError {
    #[error("failed to read spec {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("spec {path} does not parse as Gherkin: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("failed to write spec {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// One scenario row of the spec inventory.
#[derive(Debug, Clone, Serialize)]
pub struct ScenarioEntry {
    pub feature: String,
    pub scenario: String,
    pub tags: Vec<String>,
    pub line: usize,
    /// `Some(rows)` for a Scenario Outline: total Examples rows (headers
    /// excluded) — the number of concrete cases the outline expands into.
    pub outline_rows: Option<usize>,
}

/// Parse the spec file into a Gherkin feature.
///
/// # Errors
/// [`SpecError::Read`] when the file cannot be read, [`SpecError::Parse`]
/// when it is not a valid Gherkin document.
pub fn parse_spec(path: &Path) -> Result<Feature, SpecError> {
    if !path.is_file() {
        return Err(SpecError::Read {
            path: path.to_path_buf(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        });
    }
    Feature::parse_path(path, GherkinEnv::default()).map_err(|e| match e {
        gherkin::ParseFileError::Reading { path, source } => SpecError::Read { path, source },
        gherkin::ParseFileError::Parsing { path, source, .. } => SpecError::Parse {
            path,
            message: source.to_string(),
        },
    })
}

/// All scenarios of a feature, `Rule` sections flattened in document order.
fn all_scenarios(feature: &Feature) -> impl Iterator<Item = &gherkin::Scenario> {
    feature
        .scenarios
        .iter()
        .chain(feature.rules.iter().flat_map(|r| r.scenarios.iter()))
}

/// Build the scenario inventory for a parsed feature.
#[must_use]
pub fn inventory(feature: &Feature) -> Vec<ScenarioEntry> {
    all_scenarios(feature)
        .map(|s| ScenarioEntry {
            feature: feature.name.clone(),
            scenario: s.name.clone(),
            tags: s.tags.clone(),
            line: s.position.line,
            outline_rows: if s.examples.is_empty() {
                None
            } else {
                Some(
                    s.examples
                        .iter()
                        .filter_map(|e| e.table.as_ref())
                        .map(|t| t.rows.len().saturating_sub(1))
                        .sum(),
                )
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Lint
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Error,
}

/// One lint finding. `rule` is a stable machine-readable identifier.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub rule: &'static str,
    pub line: usize,
    pub message: String,
}

impl Finding {
    pub(crate) const fn error(rule: &'static str, line: usize, message: String) -> Self {
        Self {
            severity: Severity::Error,
            rule,
            line,
            message,
        }
    }

    pub(crate) const fn warning(rule: &'static str, line: usize, message: String) -> Self {
        Self {
            severity: Severity::Warning,
            rule,
            line,
            message,
        }
    }
}

/// Characters that are regex metacharacters in the runners' name filters
/// (`--name`, `swift test --filter`, `bats -f`). ADR-001 rule 4: names
/// containing these must be regex-escaped by adapters; they remain legal but
/// are correctness hazards worth flagging.
const REGEX_METACHARACTERS: &str = ".^$*+?()[]{}|";

/// Authoring lint per ADR-001's name-mangling verdict and the design doc:
/// duplicate names, code-gen-breaking characters, batch tags, missing
/// feature name are errors; regex-hostile names are warnings.
#[must_use]
pub fn lint(feature: &Feature) -> Vec<Finding> {
    let mut findings = Vec::new();

    if feature.name.trim().is_empty() {
        findings.push(Finding::error(
            "missing-feature-name",
            feature.position.line,
            "feature has no name — the feature name is the suite identity in every runner"
                .to_owned(),
        ));
    }

    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for scenario in all_scenarios(feature) {
        let name = scenario.name.as_str();
        let line = scenario.position.line;

        if name.trim().is_empty() {
            findings.push(Finding::error(
                "empty-scenario-name",
                line,
                "scenario name is empty or whitespace-only — rejected per ADR-001 \
                 (cannot become a test identifier)"
                    .to_owned(),
            ));
        } else {
            if let Some(&first) = seen.get(name) {
                findings.push(Finding::error(
                    "duplicate-scenario-name",
                    line,
                    format!(
                        "duplicate scenario name {name:?} (first defined at line {first}) — \
                         names must be unique within a feature per ADR-001"
                    ),
                ));
            }
            seen.entry(name).or_insert(line);
        }

        name_findings(name, line, &mut findings);
        for tag in &scenario.tags {
            if is_batch_tag(tag) {
                findings.push(Finding::error(
                    "batch-tag",
                    line,
                    format!(
                        "tag @{tag} encodes batching in the spec — batching lives in \
                         PLAN.md (`craftsman verify --batch` resolves the plan's \
                         Scenarios list); tags are for durable suites only"
                    ),
                ));
            }
        }
    }

    findings
}

/// Character-level checks on one scenario name: code-gen-breaking
/// characters (error) and regex-hostile ones (warning) per ADR-001.
fn name_findings(name: &str, line: usize, findings: &mut Vec<Finding>) {
    let forbidden: Vec<&str> = [
        ('`', "backtick"),
        ('\\', "backslash"),
        ('\n', "newline"),
        ('\r', "carriage return"),
    ]
    .iter()
    .filter(|(c, _)| name.contains(*c))
    .map(|&(_, label)| label)
    .collect();
    if !forbidden.is_empty() {
        findings.push(Finding::error(
            "forbidden-name-character",
            line,
            format!(
                "scenario name contains {} — rejected per ADR-001 (breaks code-gen \
                 and runner filters); rename, do not rewrite silently",
                forbidden.join(", ")
            ),
        ));
    }

    let hostile: Vec<char> = name
        .chars()
        .filter(|c| REGEX_METACHARACTERS.contains(*c))
        .collect();
    if !hostile.is_empty() {
        findings.push(Finding::warning(
            "regex-metacharacter",
            line,
            format!(
                "scenario name contains regex metacharacter(s) {hostile:?} — legal, \
                 but every runner filter must escape them (ADR-001 rule 4)"
            ),
        ));
    }
}

/// `@batch-N` (tags arrive from the parser without the `@`).
fn is_batch_tag(tag: &str) -> bool {
    tag.strip_prefix("batch-")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lint_fixture(name: &str) -> Vec<Finding> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/lint/");
        let feature = parse_spec(Path::new(&format!("{path}{name}"))).expect("fixture parses");
        lint(&feature)
    }

    fn rules(findings: &[Finding], severity: Severity) -> Vec<&'static str> {
        findings
            .iter()
            .filter(|f| f.severity == severity)
            .map(|f| f.rule)
            .collect()
    }

    #[test]
    fn clean_spec_has_no_findings() {
        assert!(lint_fixture("clean.feature").is_empty());
    }

    #[test]
    fn duplicate_scenario_names_are_errors() {
        let f = lint_fixture("duplicate-names.feature");
        assert_eq!(rules(&f, Severity::Error), vec!["duplicate-scenario-name"]);
        assert!(f[0].message.contains("Twice told"));
    }

    #[test]
    fn batch_tags_are_errors() {
        let f = lint_fixture("batch-tag.feature");
        assert_eq!(rules(&f, Severity::Error), vec!["batch-tag"]);
        assert!(f[0].message.contains("PLAN.md"));
    }

    #[test]
    fn plain_tags_are_not_batch_tags() {
        assert!(is_batch_tag("batch-12"));
        assert!(!is_batch_tag("batch-"));
        assert!(!is_batch_tag("batch-x"));
        assert!(!is_batch_tag("slow"));
        assert!(!is_batch_tag("ios-only"));
    }

    #[test]
    fn forbidden_characters_and_empty_names_are_errors() {
        let f = lint_fixture("bad-characters.feature");
        let errors = rules(&f, Severity::Error);
        assert!(errors.contains(&"forbidden-name-character"), "{f:?}");
        assert!(errors.contains(&"empty-scenario-name"), "{f:?}");
        assert!(f.iter().any(|x| x.message.contains("ADR-001")));
    }

    #[test]
    fn regex_metacharacters_are_warnings_only() {
        let f = lint_fixture("regex-metachars.feature");
        assert_eq!(rules(&f, Severity::Warning), vec!["regex-metacharacter"]);
        assert_eq!(rules(&f, Severity::Error), Vec::<&str>::new());
    }

    #[test]
    fn missing_feature_name_is_an_error() {
        let f = lint_fixture("missing-feature-name.feature");
        assert_eq!(rules(&f, Severity::Error), vec!["missing-feature-name"]);
    }

    #[test]
    fn inventory_counts_outline_rows() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/lint/clean.feature"
        );
        let feature = parse_spec(Path::new(path)).expect("fixture parses");
        let inv = inventory(&feature);
        assert_eq!(inv.len(), 3);
        assert_eq!(inv[0].outline_rows, None);
        assert_eq!(inv[2].scenario, "Checking quantities in bulk");
        assert_eq!(inv[2].outline_rows, Some(2));
        assert_eq!(inv[1].tags, vec!["slow".to_owned()]);
    }

    #[test]
    fn unreadable_spec_is_a_read_error() {
        let err = parse_spec(Path::new("/nonexistent/SPEC.md")).expect_err("must fail");
        assert!(matches!(err, SpecError::Read { .. }));
    }
}

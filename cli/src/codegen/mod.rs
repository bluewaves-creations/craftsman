//! `craftsman spec gen` — runner glue generated from SPEC.md for the
//! code-gen stacks (swift → Swift Testing per ADR-001, bash → bats per
//! ADR-002).
//!
//! Single-writer split, enforced here and nowhere else:
//! - the generated runner file (`SpecScenarios.swift` / `generated_spec.bats`)
//!   is OURS — it carries a `GENERATED` header and is fully rewritten on
//!   every run; humans never edit it;
//! - the step stub template (`Steps.swift.template` / `steps.bash.template`)
//!   is written ONCE, only if absent — after that it is THEIRS, like the
//!   real step implementations (`Steps.swift` / `steps.bash`), which this
//!   module never writes at all.
//!
//! Gen refuses to run (exit 1 at the command layer) while `spec lint` has
//! errors: every ADR-001 rejection (backticks, backslashes, newlines,
//! duplicate or empty names) would otherwise become a compile error or a
//! silent mis-selection downstream.

pub mod bash;
pub mod swift;

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::config::{Config, ConfigError, VerifyStack};
use crate::spec::{self, Severity, SpecError};

/// Errors preparing or writing generated files. Exit code 3 territory —
/// lint errors are not an error but a [`Outcome::LintErrors`] refusal.
#[derive(Debug, Error)]
pub enum GenError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Spec(#[from] SpecError),
    #[error("cannot write generated file {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "cannot locate the swift test target: {detail} — set \
         `[verify.swift] swift-tests-dir` (e.g. \"Tests/AppTests\")"
    )]
    SwiftTestsDir { detail: String },
    #[error(
        "scenario outline {scenario:?}: Examples tables disagree on headers \
         ({first:?} vs {second:?}) — align them or split the outline"
    )]
    MixedExampleHeaders {
        scenario: String,
        first: Vec<String>,
        second: Vec<String>,
    },
}

/// One generated (or deliberately skipped) file.
#[derive(Debug, serde::Serialize)]
pub struct FileReport {
    pub path: PathBuf,
    /// `written` for ours (always regenerated), `kept` for an existing step
    /// template (theirs once created — never overwritten).
    pub action: &'static str,
}

/// What `spec gen` did.
#[derive(Debug)]
pub enum Outcome {
    /// Files generated for at least one code-gen stack.
    Generated(Vec<FileReport>),
    /// Refused: the spec has lint errors (exit 1 at the command layer).
    LintErrors { errors: usize },
    /// No stack in `[project] stacks` needs code-gen (exit 4 — an empty
    /// selection is never silent success).
    NoCodegenStacks { stacks: Vec<String> },
}

/// Run `spec gen` for the project containing `cwd`.
///
/// # Errors
/// [`GenError`] on config/spec/io failures — exit code 3. A linted-red spec
/// is not an error but an [`Outcome::LintErrors`] refusal.
pub fn run(cwd: &Path) -> Result<Outcome, GenError> {
    let loaded = Config::load(cwd)?;
    let config = loaded.config;
    let root = loaded.root;

    let feature = spec::parse_spec(&root.join(&config.project.spec))?;
    let errors = spec::lint(&feature)
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    if errors > 0 {
        return Ok(Outcome::LintErrors { errors });
    }

    let codegen_stacks: Vec<&str> = config
        .project
        .stacks
        .iter()
        .map(String::as_str)
        .filter(|s| matches!(*s, "swift" | "bash"))
        .collect();
    if codegen_stacks.is_empty() {
        return Ok(Outcome::NoCodegenStacks {
            stacks: config.project.stacks.clone(),
        });
    }

    let mut files = Vec::new();
    for stack in codegen_stacks {
        let section = config.verify.stack(stack);
        match stack {
            "swift" => {
                let tests_dir = swift::resolve_tests_dir(&root, section)?;
                let out = swift::generate(&feature)?;
                write_ours(
                    &tests_dir.join("Generated/SpecScenarios.swift"),
                    &out.runner,
                    &mut files,
                )?;
                write_theirs_once(
                    &tests_dir.join("Steps.swift.template"),
                    &out.steps_template,
                    &mut files,
                )?;
            }
            "bash" => {
                let bats_dir = bats_dir(&root, section);
                let out = bash::generate(&feature)?;
                write_ours(
                    &bats_dir.join("generated_spec.bats"),
                    &out.runner,
                    &mut files,
                )?;
                write_theirs_once(
                    &bats_dir.join("steps.bash.template"),
                    &out.steps_template,
                    &mut files,
                )?;
            }
            _ => unreachable!("filtered to codegen stacks above"),
        }
    }
    Ok(Outcome::Generated(files))
}

/// The bash stack's bats directory (`[verify.bash] cwd` + `bats-dir`,
/// defaults root + `tests`).
#[must_use]
pub fn bats_dir(root: &Path, section: Option<&VerifyStack>) -> PathBuf {
    let base = section
        .and_then(|s| s.cwd.as_deref())
        .map_or_else(|| root.to_path_buf(), |c| root.join(c));
    base.join(
        section
            .and_then(|s| s.bats_dir.as_deref())
            .unwrap_or("tests"),
    )
}

/// Write one of OUR files: fully regenerated every run.
fn write_ours(path: &Path, content: &str, files: &mut Vec<FileReport>) -> Result<(), GenError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| GenError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, content).map_err(|source| GenError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    files.push(FileReport {
        path: path.to_path_buf(),
        action: "written",
    });
    Ok(())
}

/// Write one of THEIR files: once, only if absent — never overwritten
/// (the human/agent owns it from its first appearance).
fn write_theirs_once(
    path: &Path,
    content: &str,
    files: &mut Vec<FileReport>,
) -> Result<(), GenError> {
    if path.exists() {
        files.push(FileReport {
            path: path.to_path_buf(),
            action: "kept",
        });
        return Ok(());
    }
    write_ours(path, content, files)?;
    // Correct the action label: it is ours to create, theirs to keep.
    if let Some(last) = files.last_mut() {
        last.action = "created";
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared name plumbing — one slug algorithm so a step keeps the same
// function name in every generated language.
// ---------------------------------------------------------------------------

/// One outline parameter a step takes: the Examples header and whether its
/// column is integer-typed (Swift `Int` vs `String`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Param {
    pub header: String,
    pub is_int: bool,
}

/// A step reference inside one scenario: which unique step function it
/// calls and with which outline parameters (empty outside outlines).
#[derive(Debug, Clone)]
pub(crate) struct StepCall {
    /// Final function name, `step_<slug>` (de-collided).
    pub name: String,
    /// Outline parameters used by this step, in order of appearance.
    pub params: Vec<Param>,
}

/// One unique step function to stub.
#[derive(Debug, Clone)]
pub(crate) struct StepFn {
    pub name: String,
    /// Human text for the not-implemented marker: `<keyword> <value>`.
    pub display: String,
    /// Outline parameters this step takes, in order.
    pub params: Vec<Param>,
}

/// Lowercase snake slug: alphanumerics kept, everything else collapses to
/// single underscores. Never empty (falls back to `step`).
pub(crate) fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_underscore = true;
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            last_underscore = false;
        } else if !last_underscore {
            out.push('_');
            last_underscore = true;
        }
    }
    let trimmed = out.trim_end_matches('_');
    if trimmed.is_empty() {
        "step".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// The `<placeholder>` parameters present in a step's text that are outline
/// headers, in order of appearance.
fn step_params(value: &str, headers: &[Param]) -> Vec<Param> {
    let mut params: Vec<(usize, Param)> = Vec::new();
    for param in headers {
        if let Some(pos) = value.find(&format!("<{}>", param.header))
            && !params.iter().any(|(_, p)| p.header == param.header)
        {
            params.push((pos, param.clone()));
        }
    }
    params.sort_by_key(|(pos, _)| *pos);
    params.into_iter().map(|(_, p)| p).collect()
}

/// Step text with outline placeholders removed, for slugging — so every
/// Examples row calls the same function.
fn strip_placeholders(value: &str, headers: &[Param]) -> String {
    let mut out = value.to_owned();
    for param in headers {
        out = out.replace(&format!("<{}>", param.header), " ");
    }
    out
}

/// Per-feature step registry: assigns each unique step (by slug + params)
/// a stable, collision-free function name.
#[derive(Debug, Default)]
pub(crate) struct StepRegistry {
    fns: Vec<StepFn>,
}

impl StepRegistry {
    /// Register a step occurrence, returning its call site.
    pub fn call(&mut self, keyword: &str, value: &str, headers: &[Param]) -> StepCall {
        let params = step_params(value, headers);
        let base = format!("step_{}", slug(&strip_placeholders(value, headers)));
        let display = format!("{} {}", keyword.trim(), value);

        // Same slug + same params = same step function (first display wins).
        if let Some(existing) = self
            .fns
            .iter()
            .find(|f| f.params == params && (f.name == base || is_decollision_of(&f.name, &base)))
        {
            return StepCall {
                name: existing.name.clone(),
                params,
            };
        }
        // De-collide against same-named functions with different params.
        let mut name = base.clone();
        let mut n = 1;
        while self.fns.iter().any(|f| f.name == name) {
            n += 1;
            name = format!("{base}_{n}");
        }
        self.fns.push(StepFn {
            name: name.clone(),
            display,
            params: params.clone(),
        });
        StepCall { name, params }
    }

    pub fn fns(&self) -> &[StepFn] {
        &self.fns
    }
}

/// Whether `name` is `base` with a `_<n>` de-collision suffix.
fn is_decollision_of(name: &str, base: &str) -> bool {
    name.strip_prefix(base)
        .and_then(|rest| rest.strip_prefix('_'))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// A scenario outline's Examples: shared headers and every row.
#[derive(Debug, Clone)]
pub(crate) struct ExampleTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Collect a scenario's Examples rows across its tables, requiring
/// identical headers.
///
/// Returns `None` for a plain scenario (no Examples).
pub(crate) fn example_table(
    scenario: &gherkin::Scenario,
) -> Result<Option<ExampleTable>, GenError> {
    let mut merged: Option<ExampleTable> = None;
    for examples in &scenario.examples {
        let Some(table) = &examples.table else {
            continue;
        };
        let Some((headers, rows)) = table.rows.split_first() else {
            continue;
        };
        match &mut merged {
            None => {
                merged = Some(ExampleTable {
                    headers: headers.clone(),
                    rows: rows.to_vec(),
                });
            }
            Some(t) if t.headers == *headers => t.rows.extend(rows.iter().cloned()),
            Some(t) => {
                return Err(GenError::MixedExampleHeaders {
                    scenario: scenario.name.clone(),
                    first: t.headers.clone(),
                    second: headers.clone(),
                });
            }
        }
    }
    Ok(merged)
}

/// Whether every value of column `i` parses as an integer (typed columns
/// become `Int` in Swift; everything else stays a string).
pub(crate) fn column_is_int(table: &ExampleTable, i: usize) -> bool {
    !table.rows.is_empty()
        && table
            .rows
            .iter()
            .all(|row| row.get(i).is_some_and(|v| v.trim().parse::<i64>().is_ok()))
}

/// A table's headers as typed [`Param`]s.
pub(crate) fn typed_params(table: &ExampleTable) -> Vec<Param> {
    table
        .headers
        .iter()
        .enumerate()
        .map(|(i, h)| Param {
            header: h.clone(),
            is_int: column_is_int(table, i),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_collapses_and_lowercases() {
        assert_eq!(slug("An empty todo list"), "an_empty_todo_list");
        assert_eq!(slug("I add a todo \"Buy milk\""), "i_add_a_todo_buy_milk");
        assert_eq!(slug("Café — fermé!"), "café_fermé");
        assert_eq!(slug("   "), "step");
    }

    fn param(header: &str) -> Param {
        Param {
            header: header.to_owned(),
            is_int: false,
        }
    }

    #[test]
    fn registry_reuses_identical_steps_and_decollides_conflicts() {
        let mut reg = StepRegistry::default();
        let headers = vec![param("quantity")];
        let a = reg.call("Given", "an empty list", &[]);
        let b = reg.call("When", "an empty list", &[]);
        assert_eq!(a.name, b.name, "same text = same step function");
        // Same slug, different params → a distinct de-collided function.
        let c = reg.call("When", "an empty list <quantity>", &headers);
        assert_eq!(c.name, "step_an_empty_list_2");
        assert_eq!(c.params, headers);
        assert_eq!(reg.fns().len(), 2);
    }

    #[test]
    fn step_params_follow_appearance_order() {
        let headers = vec![param("reason"), param("quantity")];
        assert_eq!(
            step_params("sets <quantity> because <reason>", &headers)
                .into_iter()
                .map(|p| p.header)
                .collect::<Vec<_>>(),
            vec!["quantity".to_owned(), "reason".to_owned()]
        );
    }
}

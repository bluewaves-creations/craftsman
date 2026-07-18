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

mod a11y;
mod steps;
pub use a11y::{A11Y_STUB_FILE, a11y_stub};
pub(crate) use steps::{StepCall, StepRegistry, example_table, typed_params};

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
        generate_stack(stack, &root, &config, &feature, &mut files)?;
    }
    Ok(Outcome::Generated(files))
}

/// Generate one stack's runner file (ours, rewritten) and step template
/// (theirs, write-once).
fn generate_stack(
    stack: &str,
    root: &Path,
    config: &Config,
    feature: &gherkin::Feature,
    files: &mut Vec<FileReport>,
) -> Result<(), GenError> {
    let section = config.verify.stack(stack);
    match stack {
        "swift" => {
            let tests_dir = swift::resolve_tests_dir(root, section)?;
            let out = swift::generate(feature)?;
            write_ours(
                &tests_dir.join("Generated/SpecScenarios.swift"),
                &out.runner,
                files,
            )?;
            write_theirs_once(
                &tests_dir.join("Steps.swift.template"),
                &out.steps_template,
                files,
            )?;
        }
        "bash" => {
            let bats_dir = bats_dir(root, section);
            let out = bash::generate(feature)?;
            write_ours(&bats_dir.join("generated_spec.bats"), &out.runner, files)?;
            write_theirs_once(
                &bats_dir.join("steps.bash.template"),
                &out.steps_template,
                files,
            )?;
        }
        _ => unreachable!("filtered to codegen stacks above"),
    }
    Ok(())
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

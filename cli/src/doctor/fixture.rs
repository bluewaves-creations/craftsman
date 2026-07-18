//! The disposable rust cucumber fixture — shared by doctor's round trip
//! and the acceptance harness (`cli/tests/spec.rs`). Split from doctor's
//! check logic to keep both under the health gate's file budget.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Error writing the disposable fixture project.
#[derive(Debug, Error)]
#[error("cannot write fixture file {path}: {source}")]
pub struct ScaffoldError {
    path: PathBuf,
    #[source]
    source: std::io::Error,
}

/// Scaffold the minimal disposable rust cucumber fixture project — shared
/// by doctor's round trip and the acceptance harness (`cli/tests/spec.rs`).
///
/// `spec` is the fixture's SPEC.md; its harness implements `Given a truth`
/// and `Then it holds` (any other step is undefined by construction). The
/// `Then` step asserts `fixture::HOLDS = <holds>`, so flipping `holds`
/// only recompiles `src/lib.rs` and the cached `target/` is reused across
/// runs. Writes are skipped when content is unchanged, keeping mtimes (and
/// cargo's fingerprints) stable.
///
/// # Errors
/// [`ScaffoldError`] when a fixture file cannot be written.
pub fn scaffold_rust_fixture(dir: &Path, spec: &str, holds: bool) -> Result<(), ScaffoldError> {
    write_if_changed(
        &dir.join("craftsman.toml"),
        "[project]\nname = \"doctor-fixture\"\nstacks = [\"rust\"]\n\n[gates]\nverify = \"strict\"\n",
    )?;
    write_if_changed(&dir.join("SPEC.md"), spec)?;
    write_if_changed(
        &dir.join("Cargo.toml"),
        r#"[package]
name = "doctor-fixture"
version = "0.0.0"
edition = "2021"
publish = false

[dev-dependencies]
cucumber = { version = "0.23", features = ["output-json"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }

[[test]]
name = "spec"
harness = false

[workspace]
"#,
    )?;
    write_if_changed(
        &dir.join("tests/spec.rs"),
        r#"//! Doctor-fixture harness per the ADR-003 convention: write
//! cucumber-json to the path in CRAFTSMAN_JSON.
use cucumber::{World as _, given, then};

#[derive(Debug, Default, cucumber::World)]
struct W;

#[given("a truth")]
fn a_truth(_w: &mut W) {}

#[then("it holds")]
fn it_holds(_w: &mut W) {
    assert!(doctor_fixture::HOLDS, "the truth does not hold");
}

#[tokio::main]
async fn main() {
    let path = std::env::var("CRAFTSMAN_JSON").expect("CRAFTSMAN_JSON set by craftsman verify");
    let file = std::fs::File::create(&path).expect("create results file");
    W::cucumber()
        .with_writer(cucumber::writer::Json::new(file))
        .run("SPEC.md")
        .await;
}
"#,
    )?;
    write_truth(dir, holds)
}

pub(super) fn write_truth(dir: &Path, holds: bool) -> Result<(), ScaffoldError> {
    write_if_changed(
        &dir.join("src/lib.rs"),
        &format!("pub const HOLDS: bool = {holds};\n"),
    )
}

/// Write only when content differs, keeping mtimes stable so cargo's
/// fingerprinting reuses the cached build.
fn write_if_changed(path: &Path, content: &str) -> Result<(), ScaffoldError> {
    if std::fs::read_to_string(path).is_ok_and(|existing| existing == content) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ScaffoldError {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, content).map_err(|source| ScaffoldError {
        path: path.to_path_buf(),
        source,
    })
}

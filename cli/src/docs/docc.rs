//! `DocC` markdown export — the Swift docs source.
//!
//! Probed reality (2026-07-18, this machine, both toolchains — Xcode 26.6
//! selected and Xcode 27.0 via `DEVELOPER_DIR`):
//!
//! - `swift package generate-documentation` exists only when the package
//!   declares the `swift-docc-plugin` dependency ("Unknown subcommand or
//!   plugin name" otherwise) — it is a package plugin, not a toolchain
//!   subcommand. When available it is preferred (the research doc's
//!   recipe).
//! - `docc convert --enable-experimental-markdown-output` is present on
//!   BOTH toolchains (probed via `docc convert --help`) and emits per-page
//!   `.md` under `<archive>/data/documentation/`. So a package without the
//!   plugin still exports: `swift build` with `-emit-symbol-graph` into a
//!   private scratch dir (the source tree is never written), then
//!   `docc convert` over the symbol graphs.
//!
//! If neither path is available the sync refuses, citing the probe output
//! (the flag is experimental — observed absence must stay loud).

use std::path::{Path, PathBuf};
use std::process::Command;

use super::DocsError;

/// The flag both pipelines need (Swift 6.3's LLM-oriented export).
const MARKDOWN_FLAG: &str = "--enable-experimental-markdown-output";

/// Export a package's `DocC` as markdown pages into `pages_dir`.
/// Returns (page count, notes). `staging` holds build scratch + archive.
///
/// # Errors
/// [`DocsError`] when the package dir is missing, tools fail, or neither
/// export path supports the markdown flag.
pub fn sync(
    name: &str,
    package_dir: &Path,
    staging: &Path,
    pages_dir: &Path,
    version: &str,
) -> Result<(usize, Vec<String>), DocsError> {
    if !package_dir.join("Package.swift").is_file() {
        return Err(DocsError::LocalSourceMissing {
            path: package_dir.join("Package.swift"),
        });
    }
    let archive = staging.join("archive");
    let mut notes = Vec::new();

    let plugin_probe = run_in(
        package_dir,
        &["swift", "package", "generate-documentation", "--help"],
    );
    if plugin_probe.contains(MARKDOWN_FLAG) {
        notes.push("docc: via the package's swift-docc-plugin".to_owned());
        plugin_export(name, package_dir, staging, &archive)?;
    } else {
        notes.push(
            "docc: swift package generate-documentation unavailable here (no \
             swift-docc-plugin dependency — probed) — using swift build \
             -emit-symbol-graph + docc convert"
                .to_owned(),
        );
        direct_export(name, package_dir, staging, &archive, version)?;
    }

    let pages = harvest_markdown(&archive, pages_dir)?;
    // Only pages/ gets promoted into the cache: the build scratch, symbol
    // graphs, and archive are intermediate artifacts (the archive holds a
    // second copy of every page — search must not hit duplicates).
    for scratch in [
        &archive,
        &staging.join("build"),
        &staging.join("symbol-graphs"),
    ] {
        let _ = std::fs::remove_dir_all(scratch);
    }
    if pages == 0 {
        return Err(DocsError::DocTool {
            name: name.to_owned(),
            tool: "docc".to_owned(),
            detail: "the export produced no .md pages".to_owned(),
        });
    }
    Ok((pages, notes))
}

/// The plugin path: `swift package generate-documentation`.
fn plugin_export(
    name: &str,
    package_dir: &Path,
    staging: &Path,
    archive: &Path,
) -> Result<(), DocsError> {
    check(
        name,
        "swift package generate-documentation",
        &run_checked_in(
            package_dir,
            &[
                "swift",
                "package",
                "--allow-writing-to-directory",
                &staging.to_string_lossy(),
                "generate-documentation",
                MARKDOWN_FLAG,
                "--output-path",
                &archive.to_string_lossy(),
            ],
        ),
    )
}

/// The direct path: symbol graphs into staging, then `docc convert`.
/// Refuses (citing the probe) when `docc convert` lacks the flag.
fn direct_export(
    name: &str,
    package_dir: &Path,
    staging: &Path,
    archive: &Path,
    version: &str,
) -> Result<(), DocsError> {
    let docc_probe = run_in(package_dir, &["xcrun", "docc", "convert", "--help"]);
    if !docc_probe.contains(MARKDOWN_FLAG) {
        return Err(DocsError::DocTool {
            name: name.to_owned(),
            tool: "docc".to_owned(),
            detail: format!(
                "`docc convert --help` does not list {MARKDOWN_FLAG} on this \
                 toolchain — the experimental markdown export is unavailable \
                 (probe tail: {})",
                docc_probe.lines().last().unwrap_or_default()
            ),
        });
    }
    let graphs = staging.join("symbol-graphs");
    std::fs::create_dir_all(&graphs).map_err(|source| DocsError::Io {
        path: graphs.clone(),
        source,
    })?;
    check(
        name,
        "swift build -emit-symbol-graph",
        &run_checked_in(
            package_dir,
            &[
                "swift",
                "build",
                "--scratch-path",
                &staging.join("build").to_string_lossy(),
                "-Xswiftc",
                "-emit-symbol-graph",
                "-Xswiftc",
                "-emit-symbol-graph-dir",
                "-Xswiftc",
                &graphs.to_string_lossy(),
            ],
        ),
    )?;
    check(
        name,
        "docc convert",
        &run_checked_in(
            package_dir,
            &[
                "xcrun",
                "docc",
                "convert",
                "--additional-symbol-graph-dir",
                &graphs.to_string_lossy(),
                "--fallback-display-name",
                name,
                "--fallback-bundle-identifier",
                name,
                "--fallback-bundle-version",
                version,
                "--output-path",
                &archive.to_string_lossy(),
                MARKDOWN_FLAG,
            ],
        ),
    )
}

/// Copy every exported `.md` under the archive's `data/documentation/`
/// into `pages_dir`, path-flattened.
fn harvest_markdown(archive: &Path, pages_dir: &Path) -> Result<usize, DocsError> {
    let docs_root = archive.join("data").join("documentation");
    let mut copied = 0;
    let mut stack = vec![docs_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for p in paths {
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "md") {
                let rel = p
                    .strip_prefix(&docs_root)
                    .map_or_else(|_| p.clone(), Path::to_path_buf);
                let flat = rel.to_string_lossy().replace(['/', '\\'], "-");
                std::fs::copy(&p, pages_dir.join(&flat)).map_err(|source| DocsError::Io {
                    path: p.clone(),
                    source,
                })?;
                copied += 1;
            }
        }
    }
    Ok(copied)
}

/// Run argv in `dir`, returning combined output (empty on spawn failure) —
/// probe helper, never a verdict.
fn run_in(dir: &Path, argv: &[&str]) -> String {
    Command::new(argv[0])
        .args(&argv[1..])
        .current_dir(dir)
        .output()
        .map_or_else(
            |e| format!("(cannot spawn {}: {e})", argv[0]),
            |o| {
                format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                )
            },
        )
}

/// A tool run whose exit code matters: (label, success, output tail).
fn run_checked_in(dir: &Path, argv: &[&str]) -> (String, bool, String) {
    let label = argv.join(" ");
    match Command::new(argv[0])
        .args(&argv[1..])
        .current_dir(dir)
        .output()
    {
        Err(e) => (label, false, format!("cannot spawn: {e}")),
        Ok(o) => {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            let lines: Vec<&str> = text.lines().collect();
            let tail = lines[lines.len().saturating_sub(15)..].join("\n");
            (label, o.status.success(), tail)
        }
    }
}

fn check(name: &str, tool: &str, run: &(String, bool, String)) -> Result<(), DocsError> {
    if run.1 {
        Ok(())
    } else {
        Err(DocsError::DocTool {
            name: name.to_owned(),
            tool: tool.to_owned(),
            detail: run.2.clone(),
        })
    }
}

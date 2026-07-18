//! Vendored `.d.ts` harvest — the TypeScript docs source.
//!
//! The ground truth is already local: `tsc` checks against exactly the
//! declaration files shipped in `node_modules/<pkg>/` (research doc). Sync
//! copies every `*.d.ts` / `*.d.mts` / `*.d.cts` under the installed
//! package into the cache **verbatim** (a `.md` suffix is appended to the
//! flattened file name so the offline search covers them); the package's
//! own `package.json` version keys the cache. Fully offline — the network
//! moment was `bun install`.

use std::path::{Path, PathBuf};

use super::DocsError;

/// Harvest `node_modules/<pkg>` declarations from `project_dir` into
/// `pages_dir`. Returns (files copied, package version).
///
/// # Errors
/// [`DocsError::LocalSourceMissing`] when the package is not installed;
/// [`DocsError::Io`] on copy failure; a package with zero declaration
/// files is an error, never an empty green cache.
pub fn harvest(
    project_dir: &Path,
    pkg: &str,
    pages_dir: &Path,
    max_pages: usize,
) -> Result<(usize, Option<String>), DocsError> {
    let pkg_dir = project_dir.join("node_modules").join(pkg);
    if !pkg_dir.is_dir() {
        return Err(DocsError::LocalSourceMissing { path: pkg_dir });
    }
    let version = package_version(&pkg_dir);
    let mut copied = 0;
    let mut stack = vec![pkg_dir.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for p in paths {
            if copied >= max_pages {
                return Ok((copied, version));
            }
            if p.is_dir() {
                // A package's nested node_modules is someone else's API.
                if p.file_name().is_some_and(|n| n != "node_modules") {
                    stack.push(p);
                }
            } else if is_declaration(&p) {
                let rel = p
                    .strip_prefix(&pkg_dir)
                    .map_or_else(|_| p.clone(), Path::to_path_buf);
                let flat = rel.to_string_lossy().replace(['/', '\\'], "-") + ".md";
                std::fs::copy(&p, pages_dir.join(&flat)).map_err(|source| DocsError::Io {
                    path: p.clone(),
                    source,
                })?;
                copied += 1;
            }
        }
    }
    if copied == 0 {
        return Err(DocsError::DocTool {
            name: pkg.to_owned(),
            tool: "dts".to_owned(),
            detail: format!(
                "{} ships no .d.ts/.d.mts/.d.cts files — nothing to cache",
                pkg_dir.display()
            ),
        });
    }
    Ok((copied, version))
}

fn is_declaration(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.ends_with(".d.ts") || name.ends_with(".d.mts") || name.ends_with(".d.cts")
}

/// The installed package's own version — the authoritative cache key.
fn package_version(pkg_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(pkg_dir.join("package.json")).ok()?;
    let doc: serde_json::Value = serde_json::from_str(&text).ok()?;
    doc["version"].as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_package(root: &Path, pkg: &str, version: &str) -> PathBuf {
        let dir = root.join("node_modules").join(pkg);
        std::fs::create_dir_all(dir.join("lib/deep")).expect("mkdirs");
        std::fs::create_dir_all(dir.join("node_modules/inner")).expect("mkdirs");
        let write = |rel: &str, content: &str| {
            std::fs::write(dir.join(rel), content).expect("write");
        };
        write(
            "package.json",
            &format!("{{\"name\":\"{pkg}\",\"version\":\"{version}\"}}"),
        );
        write("index.d.ts", "export declare function parse(): void;\n");
        write("lib/deep/types.d.mts", "export type Deep = 1;\n");
        write("lib/impl.js", "module.exports = {};\n");
        write("node_modules/inner/index.d.ts", "// nested package\n");
        dir
    }

    #[test]
    fn declarations_harvest_verbatim_with_md_suffix() {
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_package(tmp.path(), "zodlike", "4.1.0");
        let pages = tmp.path().join("pages");
        std::fs::create_dir_all(&pages).expect("mkdirs");
        let (n, version) = harvest(tmp.path(), "zodlike", &pages, 200).expect("harvest");
        assert_eq!(n, 2, "js and nested node_modules excluded");
        assert_eq!(version.as_deref(), Some("4.1.0"));
        let text = std::fs::read_to_string(pages.join("index.d.ts.md")).expect("copied");
        assert_eq!(text, "export declare function parse(): void;\n", "verbatim");
        assert!(pages.join("lib-deep-types.d.mts.md").is_file(), "flattened");
    }

    #[test]
    fn missing_or_declarationless_packages_are_loud() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pages = tmp.path().join("pages");
        std::fs::create_dir_all(&pages).expect("mkdirs");
        let err = harvest(tmp.path(), "ghost", &pages, 200).expect_err("not installed");
        assert!(matches!(err, DocsError::LocalSourceMissing { .. }), "{err}");

        let dir = tmp.path().join("node_modules/plainjs");
        std::fs::create_dir_all(&dir).expect("mkdirs");
        std::fs::write(dir.join("index.js"), "x").expect("write");
        let err = harvest(tmp.path(), "plainjs", &pages, 200).expect_err("no declarations");
        assert!(err.to_string().contains("no .d.ts"), "{err}");
    }
}

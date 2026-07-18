//! Cache layout: `.craftsman/docs/<name>@<version>/pages/*.md`.
//!
//! Gitignored (licensing + size, per the documentation-pipeline research)
//! and keyed by `library@version` so a dependency bump invalidates
//! mechanically.

use std::path::{Path, PathBuf};

use super::DocsError;

/// The cache root for this project (`[docs] cache`, default
/// `.craftsman/docs`), relative paths resolved against the project root.
#[must_use]
pub fn cache_root(root: &Path, config: &crate::config::Config) -> PathBuf {
    root.join(config.docs.cache_dir())
}

/// `<cache>/<name>@<version>`.
#[must_use]
pub fn lib_dir(cache: &Path, name: &str, version: &str) -> PathBuf {
    cache.join(format!("{name}@{version}"))
}

/// The cached directory for `name` at any version, if one exists.
#[must_use]
pub fn find_lib_dir(cache: &Path, name: &str) -> Option<PathBuf> {
    let prefix = format!("{name}@");
    let entries = std::fs::read_dir(cache).ok()?;
    let mut hits: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&prefix))
        })
        .collect();
    hits.sort();
    hits.pop()
}

/// A private staging directory for one library's sync-in-progress.
#[must_use]
pub fn staging_dir(cache: &Path, name: &str) -> PathBuf {
    cache.join(format!(".staging-{name}"))
}

/// Promote a finished staging dir to `<name>@<version>`, removing every
/// older `<name>@*` copy first (one cached version per library).
///
/// # Errors
/// [`DocsError::Io`] on filesystem failure.
pub fn promote(
    cache: &Path,
    name: &str,
    version: &str,
    staging: &Path,
) -> Result<PathBuf, DocsError> {
    let prefix = format!("{name}@");
    if let Ok(entries) = std::fs::read_dir(cache) {
        for entry in entries.filter_map(Result::ok) {
            let p = entry.path();
            if p.is_dir()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&prefix))
            {
                std::fs::remove_dir_all(&p).map_err(|source| DocsError::Io { path: p, source })?;
            }
        }
    }
    let dest = lib_dir(cache, name, version);
    std::fs::rename(staging, &dest).map_err(|source| DocsError::Io {
        path: dest.clone(),
        source,
    })?;
    Ok(dest)
}

/// The page names (file stems with `.md`) cached for a library dir,
/// sorted.
#[must_use]
pub fn list_pages(lib: &Path) -> Vec<String> {
    let mut pages: Vec<String> = std::fs::read_dir(lib.join("pages"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| super::is_md(n))
        .collect();
    pages.sort();
    pages
}

/// Copy markdown from a local file or directory into `pages_dir`, bounded
/// by `max_pages`. Returns the copied page count.
///
/// # Errors
/// [`DocsError`] when the source path is missing or unreadable.
pub fn copy_local(source: &Path, pages_dir: &Path, max_pages: usize) -> Result<usize, DocsError> {
    if !source.exists() {
        return Err(DocsError::LocalSourceMissing {
            path: source.to_path_buf(),
        });
    }
    std::fs::create_dir_all(pages_dir).map_err(|source| DocsError::Io {
        path: pages_dir.to_path_buf(),
        source,
    })?;
    let mut copied = 0;
    let copy_one = |from: &Path, name: &str| -> Result<(), DocsError> {
        let mut file_name = name.replace(['/', '\\'], "-");
        if !super::is_md(&file_name) {
            file_name.push_str(".md");
        }
        std::fs::copy(from, pages_dir.join(&file_name)).map_err(|source| DocsError::Io {
            path: from.to_path_buf(),
            source,
        })?;
        Ok(())
    };
    if source.is_file() {
        let name = source
            .file_name()
            .map_or_else(|| "page".to_owned(), |n| n.to_string_lossy().into_owned());
        copy_one(source, &name)?;
        return Ok(1);
    }
    let mut stack = vec![source.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|e| DocsError::Io {
            path: dir.clone(),
            source: e,
        })?;
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for p in paths {
            if copied >= max_pages {
                return Ok(copied);
            }
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .is_some_and(|e| e == "md" || e == "markdown" || e == "txt")
            {
                let rel = p
                    .strip_prefix(source)
                    .map_or_else(|_| p.clone(), Path::to_path_buf);
                copy_one(&p, &rel.to_string_lossy())?;
                copied += 1;
            }
        }
    }
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_copy_bounds_and_flattens() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(src.join("sub")).expect("mkdirs");
        std::fs::write(src.join("a.md"), "alpha").expect("write");
        std::fs::write(src.join("sub/b.md"), "beta").expect("write");
        std::fs::write(src.join("c.rs"), "not docs").expect("write");
        let pages = tmp.path().join("pages");
        let n = copy_local(&src, &pages, 200).expect("copy");
        assert_eq!(n, 2);
        assert!(pages.join("a.md").is_file());
        assert!(pages.join("sub-b.md").is_file());
        assert!(!pages.join("c.rs.md").exists());
    }

    #[test]
    fn promote_replaces_older_versions() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cache = tmp.path();
        std::fs::create_dir_all(cache.join("lib@1.0.0/pages")).expect("old");
        let staging = staging_dir(cache, "lib");
        std::fs::create_dir_all(staging.join("pages")).expect("staging");
        std::fs::write(staging.join("pages/a.md"), "new").expect("write");
        let dest = promote(cache, "lib", "2.0.0", &staging).expect("promote");
        assert!(dest.ends_with("lib@2.0.0"));
        assert!(!cache.join("lib@1.0.0").exists(), "old version pruned");
        assert_eq!(list_pages(&dest), vec!["a.md".to_owned()]);
        assert_eq!(find_lib_dir(cache, "lib").as_deref(), Some(dest.as_path()));
        assert_eq!(find_lib_dir(cache, "other"), None);
    }
}

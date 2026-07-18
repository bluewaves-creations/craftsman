//! Offline search over the docs cache — settled design decision #4:
//! the `grep`/`ignore` crates in-process (the ripgrep internals), no FTS
//! index to maintain. Strictly offline: sync fetches, search reads.

use std::path::{Path, PathBuf};

use grep::regex::RegexMatcherBuilder;
use grep::searcher::SearcherBuilder;
use grep::searcher::sinks::UTF8;

use super::sources::Manifest;
use super::{DocsError, cache};

/// One matching line.
#[derive(Debug, serde::Serialize)]
pub struct Hit {
    pub line: u64,
    pub text: String,
}

/// All hits within one cached page, ranked by hit density.
#[derive(Debug, serde::Serialize)]
pub struct FileHits {
    /// Path relative to the cache root (`<name>@<version>/pages/…`).
    pub file: String,
    pub hits: Vec<Hit>,
    /// Hits per KiB of page — the ranking key (denser pages first).
    pub density: f64,
}

/// Search the cache (or one library's cache with `lib`) for `query` —
/// regex, smart-case (case-insensitive unless the query has an uppercase
/// letter, ripgrep's rule).
///
/// # Errors
/// [`DocsError::BadPattern`] on an invalid regex;
/// [`DocsError::UnknownLibrary`] when `lib` names an unsynced library;
/// [`DocsError::Io`] on unreadable cache files.
pub fn search(
    cache_root: &Path,
    manifest: &Manifest,
    query: &str,
    lib: Option<&str>,
) -> Result<Vec<FileHits>, DocsError> {
    let scope = match lib {
        Some(name) => {
            cache::find_lib_dir(cache_root, name).ok_or_else(|| DocsError::UnknownLibrary {
                name: name.to_owned(),
                known: known_libraries(manifest),
            })?
        }
        None => cache_root.to_path_buf(),
    };
    let matcher = RegexMatcherBuilder::new()
        .case_smart(true)
        .build(query)
        .map_err(|source| DocsError::BadPattern {
            pattern: query.to_owned(),
            detail: source.to_string(),
        })?;
    let mut searcher = SearcherBuilder::new().line_number(true).build();

    let mut results: Vec<FileHits> = Vec::new();
    for file in markdown_files(&scope) {
        let mut hits: Vec<Hit> = Vec::new();
        searcher
            .search_path(
                &matcher,
                &file,
                UTF8(|line, text| {
                    hits.push(Hit {
                        line,
                        text: text.trim_end().to_owned(),
                    });
                    Ok(true)
                }),
            )
            .map_err(|source| DocsError::Io {
                path: file.clone(),
                source,
            })?;
        if hits.is_empty() {
            continue;
        }
        let kib = std::fs::metadata(&file).map_or(1.0, |m| {
            #[expect(clippy::cast_precision_loss, reason = "page sizes are far below 2^52")]
            let b = m.len() as f64;
            (b / 1024.0).max(0.001)
        });
        #[expect(clippy::cast_precision_loss, reason = "hit counts are small")]
        let density = hits.len() as f64 / kib;
        let rel = file
            .strip_prefix(cache_root)
            .map_or_else(|_| file.clone(), Path::to_path_buf);
        results.push(FileHits {
            file: rel.to_string_lossy().into_owned(),
            hits,
            density,
        });
    }
    results.sort_by(|a, b| {
        b.density
            .partial_cmp(&a.density)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.file.cmp(&b.file))
    });
    Ok(results)
}

/// Every `.md` file under `scope`, via the `ignore` walker (sorted for
/// deterministic output).
fn markdown_files(scope: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = ignore::WalkBuilder::new(scope)
        .hidden(false)
        .build()
        .filter_map(Result::ok)
        .map(ignore::DirEntry::into_path)
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();
    files
}

fn known_libraries(manifest: &Manifest) -> Vec<String> {
    manifest.libraries.keys().cloned().collect()
}

/// Resolve `docs get <name>/<page>`: the page's cached markdown.
///
/// Offline except for ONE documented exception: an objects-inv library
/// resolves an uncached object name via its inventory and fetches the
/// target page on demand into the cache — the second get is offline.
///
/// # Errors
/// [`DocsError::BadPageSpec`] without a `/`;
/// [`DocsError::UnknownLibrary`] when the library is not in the manifest
/// or has no cache; [`DocsError::UnknownPage`] naming the available pages.
pub fn get_page(
    cache_root: &Path,
    manifest: &Manifest,
    spec: &str,
) -> Result<(String, PathBuf), DocsError> {
    let (name, page) = spec.split_once('/').ok_or_else(|| DocsError::BadPageSpec {
        spec: spec.to_owned(),
    })?;
    let Some(declared) = manifest.libraries.get(name) else {
        return Err(DocsError::UnknownLibrary {
            name: name.to_owned(),
            known: known_libraries(manifest),
        });
    };
    let lib = cache::find_lib_dir(cache_root, name).ok_or_else(|| DocsError::UnknownLibrary {
        name: name.to_owned(),
        known: known_libraries(manifest),
    })?;
    let pages = cache::list_pages(&lib);
    let wanted_md = if super::is_md(page) {
        page.to_owned()
    } else {
        format!("{page}.md")
    };
    let found = pages.iter().find(|p| **p == wanted_md).or_else(|| {
        pages
            .iter()
            .find(|p| p.contains(page.trim_end_matches(".md")))
    });
    match found {
        Some(f) => {
            let path = lib.join("pages").join(f);
            let text = std::fs::read_to_string(&path).map_err(|source| DocsError::Io {
                path: path.clone(),
                source,
            })?;
            Ok((text, path))
        }
        None if declared.source == crate::docs::sources::SourceType::ObjectsInv => {
            fetch_inventory_page(&lib, name, page, &pages)
        }
        None => Err(DocsError::UnknownPage {
            library: name.to_owned(),
            page: page.to_owned(),
            available: pages,
        }),
    }
}

/// The objects-inv on-demand path: resolve `page` as an object name in the
/// cached inventory, fetch its target page into the cache, serve it.
fn fetch_inventory_page(
    lib: &Path,
    name: &str,
    page: &str,
    pages: &[String],
) -> Result<(String, PathBuf), DocsError> {
    let unknown = || DocsError::UnknownPage {
        library: name.to_owned(),
        page: page.to_owned(),
        available: pages.to_vec(),
    };
    let inv = crate::docs::objects_inv::load(lib).ok_or_else(unknown)?;
    let entry = inv.resolve(page).ok_or_else(unknown)?;
    let url = inv.url(entry);
    // The page URL without the #fragment; fetched verbatim (usually the
    // site's HTML — data, not instructions, like everything cached here).
    let page_url = url.split('#').next().unwrap_or(&url);
    let dest = lib
        .join("pages")
        .join(crate::docs::fetch::page_slug(page_url));
    // "Cached for next time" must be true: the object name resolves to
    // the same slug deterministically, so a previous on-demand fetch is
    // served offline here (the pin for GAP-R08 caught this lookup never
    // consulting the cache — every get refetched).
    if dest.is_file() {
        let text = std::fs::read_to_string(&dest).map_err(|source| DocsError::Io {
            path: dest.clone(),
            source,
        })?;
        return Ok((text, dest));
    }
    eprintln!(
        "docs get: {} is not cached — fetching {page_url} on demand \
         (the objects-inv network exception; cached for next time)",
        entry.name
    );
    match crate::docs::fetch::fetch(page_url, &dest, &[])? {
        crate::docs::fetch::FetchStatus::Ok => {}
        status => {
            return Err(DocsError::CurlFailed {
                url: page_url.to_owned(),
                detail: format!("{status:?}"),
            });
        }
    }
    let text = std::fs::read_to_string(&dest).map_err(|source| DocsError::Io {
        path: dest.clone(),
        source,
    })?;
    Ok((text, dest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::sources::{Library, SourceType};

    fn seeded_cache() -> (tempfile::TempDir, Manifest) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pages = tmp.path().join("demo@1.0.0/pages");
        std::fs::create_dir_all(&pages).expect("mkdirs");
        std::fs::write(
            pages.join("intro.md"),
            "# Intro\n\nStreaming responses are the core feature.\nMore streaming here.\n",
        )
        .expect("write");
        std::fs::write(
            pages.join("faq.md"),
            "# FAQ\n\nNothing about that topic.\nPadding line.\nPadding line.\nPadding line.\nstreaming once.\n",
        )
        .expect("write");
        let mut manifest = Manifest::default();
        manifest.libraries.insert(
            "demo".to_owned(),
            Library {
                source: SourceType::LlmsTxt,
                urls: vec!["https://example.dev/llms.txt".to_owned()],
                path: None,
                pin: None,
                version: Some("1.0.0".to_owned()),
                fetched_at: None,
                fetched_at_epoch: None,
                sha: None,
                pages: Some(2),
            },
        );
        (tmp, manifest)
    }

    #[test]
    fn search_ranks_by_hit_density_with_line_numbers() {
        let (tmp, manifest) = seeded_cache();
        let results = search(tmp.path(), &manifest, "streaming", None).expect("search");
        assert_eq!(results.len(), 2);
        assert!(results[0].file.ends_with("intro.md"), "denser page first");
        assert_eq!(results[0].hits.len(), 2);
        assert_eq!(results[0].hits[0].line, 3);
        assert!(results[0].hits[0].text.contains("Streaming"), "smart-case");
    }

    #[test]
    fn search_smart_case_respects_uppercase_queries() {
        let (tmp, manifest) = seeded_cache();
        let results = search(tmp.path(), &manifest, "Streaming", None).expect("search");
        assert_eq!(results.len(), 1, "uppercase query is case-sensitive");
        assert!(results[0].file.ends_with("intro.md"));
    }

    #[test]
    fn search_unknown_lib_names_it() {
        let (tmp, manifest) = seeded_cache();
        let err = search(tmp.path(), &manifest, "streaming", Some("ghost"))
            .expect_err("unknown lib must fail");
        assert!(matches!(err, DocsError::UnknownLibrary { .. }), "{err}");
        assert!(err.to_string().contains("ghost"));
    }

    #[test]
    fn bad_regex_is_a_loud_error() {
        let (tmp, manifest) = seeded_cache();
        let err = search(tmp.path(), &manifest, "str(eam", None).expect_err("bad regex");
        assert!(matches!(err, DocsError::BadPattern { .. }), "{err}");
    }

    #[test]
    fn get_page_resolves_with_and_without_md_suffix() {
        let (tmp, manifest) = seeded_cache();
        let (text, _) = get_page(tmp.path(), &manifest, "demo/intro").expect("get");
        assert!(text.contains("Streaming responses"));
        let (text, _) = get_page(tmp.path(), &manifest, "demo/intro.md").expect("get");
        assert!(text.contains("Streaming responses"));
        let err = get_page(tmp.path(), &manifest, "demo/ghost").expect_err("unknown page");
        assert!(err.to_string().contains("faq.md"), "lists available: {err}");
        let err = get_page(tmp.path(), &manifest, "nosuch/intro").expect_err("unknown lib");
        assert!(err.to_string().contains("nosuch"), "{err}");
    }
}

//! `docs sync` — fetch every declared source into the version-pinned
//! cache. The docs pipeline's only network moment.

use std::path::{Path, PathBuf};

use crate::config::Config;

use super::fetch::FetchStatus;
use super::sources::{Library, Manifest, SourceType};
use super::{DocsError, SyncOutcome, cache, fetch, lockfiles, now_epoch, rustdoc};

/// Sync one library or all of them. Returns per-library outcomes; the
/// empty vec means the manifest declares no sources (exit 4 at the
/// command layer — never silent success).
///
/// # Errors
/// [`DocsError`] on the first failing library — partial state stays in
/// staging, the previous cache copy is untouched.
pub fn sync(
    root: &Path,
    config: &Config,
    only: Option<&str>,
) -> Result<Vec<SyncOutcome>, DocsError> {
    let cache_root = cache::cache_root(root, config);
    let mut manifest = Manifest::load(&cache_root)?;
    let names: Vec<String> = match only {
        Some(name) => {
            if !manifest.libraries.contains_key(name) {
                return Err(DocsError::UnknownLibrary {
                    name: name.to_owned(),
                    known: manifest.libraries.keys().cloned().collect(),
                });
            }
            vec![name.to_owned()]
        }
        None => manifest.libraries.keys().cloned().collect(),
    };
    let mut outcomes = Vec::new();
    for name in names {
        let lib = manifest.libraries[&name].clone();
        eprintln!("docs sync {name} ({}) …", lib.source);
        let outcome = sync_one(root, config, &cache_root, &name, &lib)?;
        if let Some(entry) = manifest.libraries.get_mut(&name) {
            entry.version = Some(outcome.version.clone());
            entry.fetched_at = Some(crate::gates::baseline::iso_utc_now());
            entry.fetched_at_epoch = Some(now_epoch());
            entry.pages = Some(outcome.pages);
            entry.sha = sha_of_primary(&cache_root, &name, &outcome.version);
        }
        manifest.save(&cache_root)?;
        outcomes.push(outcome);
    }
    Ok(outcomes)
}

/// sha256 of the library's primary artifact (its index or raw JSON, else
/// the first page), via the same system hasher the tool installer uses.
fn sha_of_primary(cache_root: &Path, name: &str, version: &str) -> Option<String> {
    let lib = cache::lib_dir(cache_root, name, version);
    for candidate in ["llms.txt", "rustdoc.json"] {
        let p = lib.join(candidate);
        if p.is_file() {
            return Some(crate::gates::tools::sha256(&p));
        }
    }
    let first = cache::list_pages(&lib).into_iter().next()?;
    Some(crate::gates::tools::sha256(&lib.join("pages").join(first)))
}

fn sync_one(
    root: &Path,
    config: &Config,
    cache_root: &Path,
    name: &str,
    lib: &Library,
) -> Result<SyncOutcome, DocsError> {
    if !lib.source.supported() {
        return Err(DocsError::UnsupportedSource {
            name: name.to_owned(),
            source_type: lib.source,
        });
    }
    let staging = cache::staging_dir(cache_root, name);
    let _ = std::fs::remove_dir_all(&staging);
    let pages_dir = staging.join("pages");
    std::fs::create_dir_all(&pages_dir).map_err(|source| DocsError::Io {
        path: pages_dir.clone(),
        source,
    })?;

    let max_pages = config.docs.max_pages();
    let mut fetched = Fetched::default();
    let mut version = lockfiles::resolve_lockfile_version(root, config, name)
        .or_else(|| lib.pin.clone())
        .unwrap_or_else(|| "latest".to_owned());

    match lib.source {
        SourceType::LlmsTxt => {
            let url = first_url(lib)?;
            let index_path = staging.join("llms.txt");
            expect_ok(fetch::fetch(url, &index_path, &[])?, url)?;
            let text = std::fs::read_to_string(&index_path).map_err(|source| DocsError::Io {
                path: index_path.clone(),
                source,
            })?;
            let (links, non_md) = fetch::markdown_links(url, &text);
            if links.is_empty() {
                return Err(DocsError::EmptyIndex { url: url.clone() });
            }
            fetched = fetch_pages(&links, &pages_dir, max_pages)?;
            fetched.skipped += non_md;
        }
        SourceType::PageMd => {
            fetched = fetch_pages(&lib.urls, &pages_dir, max_pages)?;
        }
        SourceType::File => {
            let src = lib
                .path
                .as_deref()
                .ok_or_else(|| DocsError::MissingLocation {
                    source_type: lib.source,
                    needs: "--path (a local markdown file or directory)".to_owned(),
                })?;
            let src_path = root.join(src);
            let from = if src_path.exists() {
                src_path
            } else {
                PathBuf::from(src)
            };
            fetched.pages = cache::copy_local(&from, &pages_dir, max_pages)?;
        }
        SourceType::DocsrsJson => {
            sync_docsrs(name, &mut version, &staging, &pages_dir)?;
            fetched.pages = 1;
        }
        SourceType::Context7 => {
            let id = first_url(lib)?;
            let key = std::env::var("CONTEXT7_API_KEY").ok();
            let (url, headers) = fetch::context7_request(id, name, key.as_deref());
            expect_ok(
                fetch::fetch(&url, &pages_dir.join("context7.md"), &headers)?,
                &url,
            )?;
            fetched.pages = 1;
        }
        SourceType::Docc | SourceType::ObjectsInv | SourceType::Dts => {
            unreachable!("refused above via SourceType::supported")
        }
    }

    cache::promote(cache_root, name, &version, &staging)?;
    Ok(SyncOutcome {
        name: name.to_owned(),
        source: lib.source,
        version,
        pages: fetched.pages,
        skipped: fetched.skipped,
        notes: fetched.notes,
    })
}

/// Page-fetch tally for one library.
#[derive(Debug, Default)]
struct Fetched {
    pages: usize,
    skipped: usize,
    notes: Vec<String>,
}

/// The CLI-written manifest guarantees a URL for url-bearing sources; a
/// hand-edited manifest without one is an error, not a panic.
fn first_url(lib: &Library) -> Result<&String, DocsError> {
    lib.urls.first().ok_or_else(|| DocsError::MissingLocation {
        source_type: lib.source,
        needs: "--url (re-run `craftsman docs add`)".to_owned(),
    })
}

/// Fetch up to `max_pages` page URLs into `pages_dir`, counting the rest.
fn fetch_pages(links: &[String], pages_dir: &Path, max_pages: usize) -> Result<Fetched, DocsError> {
    let mut tally = Fetched::default();
    if links.len() > max_pages {
        tally.notes.push(format!(
            "listing has {} pages; fetching the first {max_pages} ([docs] max-pages)",
            links.len()
        ));
    }
    for link in links.iter().take(max_pages) {
        match fetch::fetch(link, &pages_dir.join(fetch::page_slug(link)), &[])? {
            FetchStatus::Ok => tally.pages += 1,
            status => {
                tally.skipped += 1;
                tally.notes.push(format!("skipped {link}: {status:?}"));
            }
        }
    }
    tally.skipped += links.len().saturating_sub(max_pages);
    Ok(tally)
}

/// Fetch + decompress + render the docs.rs rustdoc JSON; the JSON's own
/// `crate_version` becomes the authoritative cache version.
fn sync_docsrs(
    name: &str,
    version: &mut String,
    staging: &Path,
    pages_dir: &Path,
) -> Result<(), DocsError> {
    let url = format!("https://docs.rs/crate/{name}/{version}/json.gz");
    let gz = staging.join("rustdoc.json.gz");
    expect_ok(fetch::fetch(&url, &gz, &[])?, &url)?;
    let raw = staging.join("rustdoc.json");
    fetch::gunzip(&gz, &raw)?;
    let text = std::fs::read_to_string(&raw).map_err(|source| DocsError::Io {
        path: raw.clone(),
        source,
    })?;
    let doc: serde_json::Value =
        serde_json::from_str(&text).map_err(|source| DocsError::ManifestParse {
            path: raw.clone(),
            source,
        })?;
    if let Some(v) = doc["crate_version"].as_str() {
        v.clone_into(version);
    }
    let md = rustdoc::render_rustdoc_md(&doc, name);
    let api = pages_dir.join("api.md");
    std::fs::write(&api, md).map_err(|source| DocsError::Io { path: api, source })?;
    let _ = std::fs::remove_file(&gz);
    Ok(())
}

/// A mandatory fetch (index, JSON artifact): every non-OK status is fatal.
fn expect_ok(status: FetchStatus, url: &str) -> Result<(), DocsError> {
    match status {
        FetchStatus::Ok => Ok(()),
        FetchStatus::RateLimited => Err(DocsError::RateLimited {
            url: url.to_owned(),
        }),
        FetchStatus::NotFound => Err(DocsError::HttpStatus {
            url: url.to_owned(),
            status: 404,
        }),
        FetchStatus::Http(status) => Err(DocsError::HttpStatus {
            url: url.to_owned(),
            status,
        }),
        FetchStatus::TooLarge => Err(DocsError::CurlFailed {
            url: url.to_owned(),
            detail: format!(
                "response exceeds the {}-byte page cap",
                fetch::MAX_PAGE_BYTES
            ),
        }),
    }
}

//! `craftsman docs` — the documentation grounding pipeline.
//!
//! Design (documentation-pipeline research + surface design): sources are
//! declared once (`docs add` → `.craftsman/docs/manifest.json`, CLI-written,
//! single-writer; the AGENTS.md table stays human-owned — see `sources`),
//! fetched by `docs sync` (the only network moment), and consumed offline
//! by `docs search`/`docs get` over the version-pinned markdown cache.
//!
//! Fetched documentation is **data, not instructions** (the `ContextCrush`
//! precedent): stored verbatim, never interpreted, and the read commands
//! print a one-line stderr notice per run saying exactly that.

pub mod cache;
pub mod fetch;
pub mod lockfiles;
pub mod rustdoc;
pub mod search;
pub mod sources;
mod sync;

pub use sync::sync;

use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::config::{Config, ConfigError};
use sources::{Library, Manifest, SourceType};

/// Case-insensitive `.md` suffix check for page names and URLs.
#[must_use]
pub fn is_md(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// Errors of the docs pipeline. Exit code 3 territory at the command layer.
#[derive(Debug, Error)]
pub enum DocsError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("cannot read or write {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid docs manifest at {path}: {source} — fix or delete it")]
    ManifestParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to run curl for {url}")]
    CurlSpawn {
        url: String,
        #[source]
        source: std::io::Error,
    },
    #[error("fetch of {url} failed: {detail}")]
    CurlFailed { url: String, detail: String },
    #[error(
        "rate limited (HTTP 429) by {url} — keyless Context7 access is \
         low-rate; set CONTEXT7_API_KEY for the authenticated tier or retry later"
    )]
    RateLimited { url: String },
    #[error("fetch of {url} returned HTTP {status}")]
    HttpStatus { url: String, status: u16 },
    #[error(
        "unknown library \"{name}\" — not in the docs manifest or never \
         synced; known: {}; run `craftsman docs add`/`docs sync` first",
        if known.is_empty() { "(none)".to_owned() } else { known.join(", ") }
    )]
    UnknownLibrary { name: String, known: Vec<String> },
    #[error(
        "library \"{library}\" has no cached page \"{page}\"; available: {}",
        if available.is_empty() { "(none — run `craftsman docs sync`)".to_owned() } else { available.join(", ") }
    )]
    UnknownPage {
        library: String,
        page: String,
        available: Vec<String>,
    },
    #[error("page spec \"{spec}\" must be <library>/<page> (see `craftsman docs get --help`)")]
    BadPageSpec { spec: String },
    #[error("invalid search pattern \"{pattern}\": {detail}")]
    BadPattern { pattern: String, detail: String },
    #[error(
        "source \"{source_type}\" for \"{name}\" is not yet supported by docs sync \
         (accepted at add-time so the manifest format is stable; docc, \
         objects-inv, and dts land in a later batch)"
    )]
    UnsupportedSource {
        name: String,
        source_type: SourceType,
    },
    #[error("source type {source_type} requires {needs}")]
    MissingLocation {
        source_type: SourceType,
        needs: String,
    },
    #[error("local docs source {path} does not exist")]
    LocalSourceMissing { path: PathBuf },
    #[error("llms.txt index at {url} lists no per-page .md links — nothing to cache")]
    EmptyIndex { url: String },
}

/// What `docs add` recorded.
#[derive(Debug, Serialize)]
pub struct AddReport {
    pub name: String,
    pub source: SourceType,
    /// A human nudge about the AGENTS.md Documentation Sources table
    /// (which the CLI deliberately never edits).
    pub agents_note: Option<String>,
}

/// One library's `docs sync` result.
#[derive(Debug, Serialize)]
pub struct SyncOutcome {
    pub name: String,
    pub source: SourceType,
    pub version: String,
    pub pages: usize,
    /// Pages skipped (non-md links, 404s, over the size cap, page bound).
    pub skipped: usize,
    pub notes: Vec<String>,
}

/// One `docs status` row.
#[derive(Debug, Serialize)]
pub struct StatusRow {
    pub name: String,
    pub source: SourceType,
    pub cached_version: Option<String>,
    pub lockfile_version: Option<String>,
    /// Cached version disagrees with the lockfile pin — resync advised.
    pub drift: bool,
    pub fetched_at: Option<String>,
    pub age_days: Option<u64>,
    pub pages: Option<usize>,
    pub agents_note: Option<String>,
}

/// Register (or redefine) a documentation source in the manifest.
///
/// No network: `docs add` writes the declaration; `docs sync` fetches.
///
/// # Errors
/// [`DocsError::MissingLocation`] when the source type needs a URL/path it
/// was not given; manifest I/O errors otherwise.
pub fn add(
    root: &Path,
    config: &Config,
    name: &str,
    source: SourceType,
    urls: &[String],
    path: Option<&str>,
    pin: Option<&str>,
) -> Result<AddReport, DocsError> {
    match source {
        SourceType::LlmsTxt | SourceType::PageMd | SourceType::Context7 if urls.is_empty() => {
            return Err(DocsError::MissingLocation {
                source_type: source,
                needs: "--url (the index URL, page URLs, or Context7 library id)".to_owned(),
            });
        }
        SourceType::File if path.is_none() => {
            return Err(DocsError::MissingLocation {
                source_type: source,
                needs: "--path (a local markdown file or directory)".to_owned(),
            });
        }
        _ => {}
    }
    let cache_root = cache::cache_root(root, config);
    let mut manifest = Manifest::load(&cache_root)?;
    manifest.libraries.insert(
        name.to_owned(),
        Library {
            source,
            urls: urls.to_vec(),
            path: path.map(str::to_owned),
            pin: pin.map(str::to_owned),
            version: None,
            fetched_at: None,
            fetched_at_epoch: None,
            sha: None,
            pages: None,
        },
    );
    manifest.save(&cache_root)?;
    Ok(AddReport {
        name: name.to_owned(),
        source,
        agents_note: agents_note(root, name),
    })
}

fn agents_note(root: &Path, name: &str) -> Option<String> {
    match sources::agents_md_row(root, name) {
        Some(false) => Some(format!(
            "AGENTS.md has a Documentation Sources table without a row for \
             \"{name}\" — add one (the table is human-owned; the CLI never edits it)"
        )),
        _ => None,
    }
}

/// Seconds since the unix epoch, for fetch-age math.
pub(crate) fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Manifest vs lockfiles staleness report — advice, never a gate.
///
/// # Errors
/// Manifest I/O errors only; missing lockfiles are simply "unknown".
pub fn status(root: &Path, config: &Config) -> Result<Vec<StatusRow>, DocsError> {
    let cache_root = cache::cache_root(root, config);
    let manifest = Manifest::load(&cache_root)?;
    let now = now_epoch();
    Ok(manifest
        .libraries
        .iter()
        .map(|(name, lib)| {
            let lockfile_version = lockfiles::resolve_lockfile_version(root, config, name);
            let drift = match (&lib.version, &lockfile_version) {
                (Some(cached), Some(locked)) => cached != locked,
                _ => false,
            };
            StatusRow {
                name: name.clone(),
                source: lib.source,
                cached_version: lib.version.clone(),
                lockfile_version,
                drift,
                fetched_at: lib.fetched_at.clone(),
                age_days: lib
                    .fetched_at_epoch
                    .map(|then| now.saturating_sub(then) / 86_400),
                pages: lib.pages,
                agents_note: agents_note(root, name),
            }
        })
        .collect())
}

//! Documentation sources.
//!
//! The closed source-type enum, the CLI-written manifest at
//! `.craftsman/docs/manifest.json`, and the AGENTS.md table check
//! (lockfile parsing lives in `lockfiles`).
//!
//! **Settled decision (Batch 7):** sources persist in the manifest only —
//! the CLI never edits the AGENTS.md Documentation Sources table. That
//! table is the *human-owned declaration* (research doc: "the human
//! declares sources once"); a CLI appending rows to prose markdown is both
//! fragile and an ownership violation. Instead `docs add`/`docs status`
//! print a note when AGENTS.md has a Documentation Sources table without a
//! row for the library, so the human keeps the declaration current.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::DocsError;

/// The closed source-type enum from the design doc — all eight implemented
/// (docc/objects-inv/dts landed in Batch 9b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[value(rename_all = "kebab-case")]
pub enum SourceType {
    /// An `llms.txt`-style markdown index whose links are fetched per page.
    LlmsTxt,
    /// An explicit list of per-page markdown URLs.
    PageMd,
    /// A local file or directory of markdown, copied into the cache.
    File,
    /// docs.rs prebuilt rustdoc JSON, stored raw plus a markdown rendering.
    DocsrsJson,
    /// Context7 REST v2 aggregator (keyless low-rate; `CONTEXT7_API_KEY`).
    Context7,
    /// `DocC` markdown export of a Swift package (`--path` = package dir).
    Docc,
    /// Sphinx objects.inv inventory (`--url` = the objects.inv URL);
    /// `docs get` fetches target pages on demand — the one documented
    /// network exception outside sync.
    ObjectsInv,
    /// Vendored `node_modules/<name>/**/*.d.ts`, cached verbatim
    /// (`--path` = the project directory).
    Dts,
}

impl fmt::Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LlmsTxt => "llms-txt",
            Self::PageMd => "page-md",
            Self::File => "file",
            Self::DocsrsJson => "docsrs-json",
            Self::Context7 => "context7",
            Self::Docc => "docc",
            Self::ObjectsInv => "objects-inv",
            Self::Dts => "dts",
        })
    }
}

/// One declared library source plus its last-sync state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Library {
    pub source: SourceType,
    /// Remote locations: the index URL (llms-txt), the page list (page-md),
    /// or the Context7 library id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
    /// Local path (file source only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Human version pin from `docs add --pin` (e.g. "4.x").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    /// Resolved version of the cached copy (`<name>@<version>` dir key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    /// Same instant as `fetched_at`, as a unix epoch for age math.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at_epoch: Option<u64>,
    /// sha256 of the primary fetched artifact (index / rustdoc JSON).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<usize>,
}

/// `.craftsman/docs/manifest.json` — CLI-written (single-writer).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    pub libraries: BTreeMap<String, Library>,
}

impl Manifest {
    /// Load the manifest under `cache_dir`, or an empty one when absent.
    ///
    /// # Errors
    /// [`DocsError`] on unreadable or invalid JSON — a corrupt manifest is
    /// never silently reset.
    pub fn load(cache_dir: &Path) -> Result<Self, DocsError> {
        let path = cache_dir.join("manifest.json");
        if !path.is_file() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path).map_err(|source| DocsError::Io {
            path: path.clone(),
            source,
        })?;
        serde_json::from_str(&text).map_err(|source| DocsError::ManifestParse { path, source })
    }

    /// Write the manifest under `cache_dir`, creating the directory.
    ///
    /// # Errors
    /// [`DocsError::Io`] on filesystem failure.
    pub fn save(&self, cache_dir: &Path) -> Result<(), DocsError> {
        std::fs::create_dir_all(cache_dir).map_err(|source| DocsError::Io {
            path: cache_dir.to_path_buf(),
            source,
        })?;
        let path = cache_dir.join("manifest.json");
        let text = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_owned());
        std::fs::write(&path, text + "\n").map_err(|source| DocsError::Io { path, source })
    }
}

/// Does the AGENTS.md Documentation Sources table (when present) carry a
/// row mentioning `name`? `None` = no AGENTS.md or no such table.
#[must_use]
pub fn agents_md_row(root: &Path, name: &str) -> Option<bool> {
    let text = std::fs::read_to_string(root.join("AGENTS.md")).ok()?;
    let mut in_section = false;
    let mut saw_table = false;
    for line in text.lines() {
        if line.starts_with('#') {
            in_section = line
                .trim_start_matches('#')
                .trim()
                .eq_ignore_ascii_case("Documentation Sources");
            continue;
        }
        if in_section && line.trim_start().starts_with('|') {
            saw_table = true;
            if line
                .to_ascii_lowercase()
                .contains(&name.to_ascii_lowercase())
            {
                return Some(true);
            }
        }
    }
    saw_table.then_some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trips() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut m = Manifest::default();
        m.libraries.insert(
            "clap".to_owned(),
            Library {
                source: SourceType::DocsrsJson,
                urls: Vec::new(),
                path: None,
                pin: Some("4".to_owned()),
                version: Some("4.6.2".to_owned()),
                fetched_at: None,
                fetched_at_epoch: None,
                sha: None,
                pages: None,
            },
        );
        m.save(tmp.path()).expect("save");
        let back = Manifest::load(tmp.path()).expect("load");
        assert_eq!(back.libraries["clap"].source, SourceType::DocsrsJson);
        assert_eq!(back.libraries["clap"].pin.as_deref(), Some("4"));
    }

    #[test]
    fn corrupt_manifest_is_a_loud_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("manifest.json"), "{nope").expect("write");
        let err = Manifest::load(tmp.path()).expect_err("corrupt manifest must error");
        assert!(matches!(err, DocsError::ManifestParse { .. }), "{err}");
    }

    #[test]
    fn agents_md_table_lookup() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("AGENTS.md"),
            "# Repo\n\n## Documentation Sources\n\n| Library | Source |\n|---|---|\n| clap | docsrs |\n\n## Other\n",
        )
        .expect("write");
        assert_eq!(agents_md_row(tmp.path(), "clap"), Some(true));
        assert_eq!(agents_md_row(tmp.path(), "tokio"), Some(false));
        let empty = tempfile::tempdir().expect("tempdir");
        assert_eq!(agents_md_row(empty.path(), "clap"), None);
    }
}

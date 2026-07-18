//! Sphinx `objects.inv` (inventory version 2) — the Python docs index.
//!
//! Format (verified live against `https://docs.pydantic.dev/latest/objects.inv`,
//! 2026-07-18): four `#`-prefixed header lines (version marker, project,
//! version, compression note), then a single zlib stream of
//! `name domain:role priority uri dispname` lines. A `$` ending the uri is
//! shorthand for the object name; a `-` dispname means "same as name".
//!
//! Sync caches the **index**: `pages/inventory.md` (one line per object,
//! name → resolved URL) plus `inventory.json` beside it for machine
//! resolution. `docs search` covers the index offline. `docs get` on an
//! objects-inv library is the pipeline's ONE documented network exception:
//! target pages are HTML sites, so they are fetched on demand (and cached —
//! the second get is offline). Decided Batch 9b over prefetching top-level
//! pages: an unbounded HTML crawl is neither markdown nor bounded.

use std::fmt::Write as _;
use std::io::Read as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::DocsError;

/// One inventory object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    /// `domain:role`, e.g. `py:function`.
    pub role: String,
    /// URI relative to the inventory's base URL, `$` already expanded.
    pub uri: String,
}

/// A parsed inventory: header metadata + entries.
#[derive(Debug, Serialize, Deserialize)]
pub struct Inventory {
    pub project: String,
    pub version: String,
    /// The inventory URL minus `objects.inv` — entry URIs resolve here.
    pub base_url: String,
    pub entries: Vec<Entry>,
}

impl Inventory {
    /// The absolute URL of one entry.
    #[must_use]
    pub fn url(&self, entry: &Entry) -> String {
        format!("{}{}", self.base_url, entry.uri)
    }

    /// Resolve a name to an entry: exact match first, else the first
    /// entry whose name contains `name`.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|e| e.name == name)
            .or_else(|| self.entries.iter().find(|e| e.name.contains(name)))
    }
}

/// Parse `objects.inv` bytes fetched from `url`.
///
/// # Errors
/// [`DocsError::InventoryFormat`] on anything but a well-formed v2
/// inventory — a truncated or foreign file is never an empty index.
pub fn parse(url: &str, bytes: &[u8]) -> Result<Inventory, DocsError> {
    let bad = |detail: &str| DocsError::InventoryFormat {
        url: url.to_owned(),
        detail: detail.to_owned(),
    };
    let mut offset = 0;
    let mut header: Vec<String> = Vec::new();
    for _ in 0..4 {
        let nl = bytes[offset..]
            .iter()
            .position(|b| *b == b'\n')
            .ok_or_else(|| bad("fewer than 4 header lines"))?;
        header.push(String::from_utf8_lossy(&bytes[offset..offset + nl]).into_owned());
        offset += nl + 1;
    }
    if header[0].trim() != "# Sphinx inventory version 2" {
        return Err(bad(&format!(
            "not a v2 inventory (first line: {:?})",
            header[0]
        )));
    }
    let mut inflated = String::new();
    flate2::read::ZlibDecoder::new(&bytes[offset..])
        .read_to_string(&mut inflated)
        .map_err(|e| bad(&format!("zlib payload does not inflate: {e}")))?;

    let strip = |line: &str, prefix: &str| line.strip_prefix(prefix).map(str::to_owned);
    let project = strip(&header[1], "# Project: ").ok_or_else(|| bad("no Project header"))?;
    let version = strip(&header[2], "# Version: ").ok_or_else(|| bad("no Version header"))?;
    let base_url = url.rsplit_once('/').map_or("", |(dir, _)| dir).to_owned() + "/";

    let entries: Vec<Entry> = inflated.lines().filter_map(parse_entry).collect();
    if entries.is_empty() {
        return Err(bad("inventory inflates to zero entries"));
    }
    Ok(Inventory {
        project,
        version,
        base_url,
        entries,
    })
}

/// One `name domain:role priority uri dispname` line. Names may contain
/// spaces (std-domain labels), so the anchor is the `domain:role` token
/// followed by an integer priority — sphobjinv's grammar.
fn parse_entry(line: &str) -> Option<Entry> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let anchor = tokens.iter().enumerate().position(|(i, t)| {
        i > 0
            && t.contains(':')
            && tokens.get(i + 1).is_some_and(|p| p.parse::<i64>().is_ok())
            && tokens.len() > i + 2
    })?;
    let name = tokens[..anchor].join(" ");
    let uri = tokens[anchor + 2].to_owned();
    let uri = uri
        .strip_suffix('$')
        .map_or_else(|| uri.clone(), |head| format!("{head}{name}"));
    Some(Entry {
        name,
        role: tokens[anchor].to_owned(),
        uri,
    })
}

/// Render the searchable index page: one line per object.
#[must_use]
pub fn render_index_md(inv: &Inventory) -> String {
    let mut out = format!(
        "# {} {} — Sphinx object inventory\n\n{} objects. `docs get <lib>/<object-name>` \
         fetches a page on demand (the documented objects-inv network exception).\n\n",
        inv.project,
        inv.version,
        inv.entries.len()
    );
    for e in &inv.entries {
        let _ = writeln!(out, "- `{}` ({}) — {}", e.name, e.role, inv.url(e));
    }
    out
}

/// Load `inventory.json` from a library cache dir.
#[must_use]
pub fn load(lib_dir: &Path) -> Option<Inventory> {
    let text = std::fs::read_to_string(lib_dir.join("inventory.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// Write `inventory.json` into a (staging) library dir.
///
/// # Errors
/// [`DocsError::Io`] on filesystem failure.
pub fn save(lib_dir: &Path, inv: &Inventory) -> Result<(), DocsError> {
    let path = lib_dir.join("inventory.json");
    let text = serde_json::to_string(inv).map_err(|e| DocsError::InventoryFormat {
        url: inv.base_url.clone(),
        detail: format!("cannot serialize inventory: {e}"),
    })?;
    std::fs::write(&path, text).map_err(|source| DocsError::Io { path, source })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    /// A tiny inventory in the exact live format (entry lines match the
    /// pydantic capture, 2026-07-18).
    fn sample_bytes() -> Vec<u8> {
        let header = "# Sphinx inventory version 2\n# Project: Pydantic\n# Version: 0.0.0\n# The remainder of this file is compressed using zlib.\n";
        let body = "pydantic.functional_validators.model_validator py:function 1 api/pydantic/functional_validators/#$ -\npydantic.model_validator py:function 1 api/pydantic/functional_validators/#pydantic.functional_validators.model_validator -\nPEP 484 std:label -1 pep-0484/ PEP 484 title\n";
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(body.as_bytes()).expect("compress");
        let mut bytes = header.as_bytes().to_vec();
        bytes.extend(enc.finish().expect("finish"));
        bytes
    }

    #[test]
    fn v2_inventory_parses_with_dollar_expansion_and_spaced_names() {
        let inv = parse(
            "https://docs.pydantic.dev/latest/objects.inv",
            &sample_bytes(),
        )
        .expect("parses");
        assert_eq!(inv.project, "Pydantic");
        assert_eq!(inv.base_url, "https://docs.pydantic.dev/latest/");
        assert_eq!(inv.entries.len(), 3);
        let first = &inv.entries[0];
        assert_eq!(
            first.uri,
            "api/pydantic/functional_validators/#pydantic.functional_validators.model_validator",
            "$ expands to the name"
        );
        assert_eq!(inv.entries[2].name, "PEP 484", "spaced names survive");
        assert_eq!(inv.entries[2].role, "std:label");
        let url = inv.url(inv.resolve("pydantic.model_validator").expect("resolves"));
        assert!(url.starts_with("https://docs.pydantic.dev/latest/api/"));
        assert!(
            inv.resolve("model_validator").is_some(),
            "substring fallback"
        );
        assert!(inv.resolve("no_such_thing_at_all").is_none());
    }

    #[test]
    fn index_page_lists_every_object_with_its_url() {
        let inv = parse("https://x.dev/latest/objects.inv", &sample_bytes()).expect("parses");
        let md = render_index_md(&inv);
        assert!(md.contains("3 objects"));
        assert!(md.contains(
            "`pydantic.model_validator` (py:function) — https://x.dev/latest/api/pydantic/functional_validators/#pydantic.functional_validators.model_validator"
        ));
    }

    #[test]
    fn foreign_or_truncated_bytes_are_loud_errors() {
        let err = parse("https://x.dev/objects.inv", b"<!doctype html>").expect_err("html");
        assert!(matches!(err, DocsError::InventoryFormat { .. }), "{err}");
        let err = parse(
            "https://x.dev/objects.inv",
            b"# Sphinx inventory version 2\n# Project: X\n# Version: 1\n# zlib\nnot-zlib",
        )
        .expect_err("bad payload");
        assert!(err.to_string().contains("inflate"), "{err}");
    }

    #[test]
    fn inventory_round_trips_through_the_cache_dir() {
        let inv = parse("https://x.dev/objects.inv", &sample_bytes()).expect("parses");
        let tmp = tempfile::tempdir().expect("tempdir");
        save(tmp.path(), &inv).expect("save");
        let back = load(tmp.path()).expect("load");
        assert_eq!(back.entries.len(), 3);
        assert_eq!(back.project, "Pydantic");
        assert!(load(Path::new("/nonexistent")).is_none());
    }
}

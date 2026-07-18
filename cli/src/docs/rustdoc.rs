//! docs.rs prebuilt rustdoc JSON → markdown, kept deliberately small
//! (Batch 7 scope): item paths and doc strings only — signatures and
//! cross-references stay in the raw `rustdoc.json` cached alongside.

use std::fmt::Write as _;

/// A minimal markdown rendering: every local item that has a `docs`
/// string, addressed by its full path.
///
/// Schema: `format_version` 60 as served live by docs.rs on 2026-07-18
/// (`paths` id → `{crate_id, path, kind}`; `index` id → `{name, docs}`).
#[must_use]
pub fn render_rustdoc_md(doc: &serde_json::Value, crate_name: &str) -> String {
    let version = doc["crate_version"].as_str().unwrap_or("unknown");
    let mut out =
        format!("# {crate_name} {version} — API docs (rendered from docs.rs rustdoc JSON)\n");
    let empty = serde_json::Map::new();
    let paths = doc["paths"].as_object().unwrap_or(&empty);
    let index = doc["index"].as_object().unwrap_or(&empty);

    let mut entries: Vec<(String, &str, &str)> = Vec::new();
    for (id, meta) in paths {
        // crate_id 0 = the crate itself; foreign items add noise.
        if meta["crate_id"].as_u64() != Some(0) {
            continue;
        }
        let Some(docs) = index.get(id).and_then(|item| item["docs"].as_str()) else {
            continue;
        };
        if docs.trim().is_empty() {
            continue;
        }
        let path: Vec<&str> = meta["path"]
            .as_array()
            .map(|segs| segs.iter().filter_map(|s| s.as_str()).collect())
            .unwrap_or_default();
        let kind = meta["kind"].as_str().unwrap_or("item");
        entries.push((path.join("::"), kind, docs));
    }
    entries.sort_unstable();
    for (path, kind, docs) in entries {
        let _ = write!(out, "\n## {path} ({kind})\n\n{docs}\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_lists_local_documented_items_by_path() {
        // Constructed to the format_version-60 shape observed live from
        // https://docs.rs/crate/gherkin/latest/json.gz on 2026-07-18.
        let doc = serde_json::json!({
            "crate_version": "0.16.0",
            "paths": {
                "1": {"crate_id": 0, "path": ["gherkin", "Feature"], "kind": "struct"},
                "2": {"crate_id": 0, "path": ["gherkin", "undocd"], "kind": "fn"},
                "3": {"crate_id": 2, "path": ["core", "fmt", "Debug"], "kind": "trait"},
            },
            "index": {
                "1": {"name": "Feature", "docs": "A feature."},
                "2": {"name": "undocd", "docs": ""},
                "3": {"name": "Debug", "docs": "Foreign."},
            },
        });
        let md = render_rustdoc_md(&doc, "gherkin");
        assert!(md.contains("# gherkin 0.16.0"));
        assert!(md.contains("## gherkin::Feature (struct)\n\nA feature."));
        assert!(!md.contains("undocd"), "empty docs are omitted");
        assert!(!md.contains("core::fmt"), "foreign crates are omitted");
    }
}

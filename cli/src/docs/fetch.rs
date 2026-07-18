//! Fetchers for `docs sync` — the only network moment of the docs pipeline
//! (searches and gets are strictly offline reads of the cache).
//!
//! Network goes through the system `curl`, the same transport the gate
//! tool installer uses (`gates::tools`): no HTTP crate to vet, TLS from the
//! OS, and one consistent place to reason about network behavior.
//!
//! Injection posture (documentation-pipeline research, the `ContextCrush`
//! precedent): everything fetched here is **data, not instructions** — it
//! is stored verbatim and never interpreted; the read commands print a
//! notice to that effect.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use super::DocsError;

/// Hard size cap per fetched page (llms-full.txt at platform scale is
/// 57–90 MB — a corpus, not a page).
pub const MAX_PAGE_BYTES: u64 = 2 * 1024 * 1024;

/// What one HTTP GET produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchStatus {
    Ok,
    NotFound,
    RateLimited,
    /// Any other non-2xx status.
    Http(u16),
    /// Response exceeded [`MAX_PAGE_BYTES`].
    TooLarge,
}

/// GET `url` to `dest` via curl. Extra headers are passed as `-H` pairs.
///
/// # Errors
/// [`DocsError::CurlSpawn`]/[`DocsError::CurlFailed`] when curl itself
/// cannot run — HTTP-level failures are a [`FetchStatus`], not an error,
/// so per-page 404s in a bounded crawl stay countable.
pub fn fetch(url: &str, dest: &Path, headers: &[String]) -> Result<FetchStatus, DocsError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|source| DocsError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let output = Command::new("curl")
        .args(curl_args(url, dest, headers))
        .output()
        .map_err(|source| DocsError::CurlSpawn {
            url: url.to_owned(),
            source,
        })?;
    // curl exit 63 = --max-filesize exceeded; the page is skipped, loudly.
    if output.status.code() == Some(63) {
        let _ = std::fs::remove_file(dest);
        return Ok(FetchStatus::TooLarge);
    }
    let code: u16 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    if !output.status.success() && code == 0 {
        return Err(DocsError::CurlFailed {
            url: url.to_owned(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(classify(code, dest))
}

/// The curl invocation: quiet, redirect-following, bounded in time and
/// size, with the HTTP status on stdout.
fn curl_args(url: &str, dest: &Path, headers: &[String]) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-sSL".into(),
        "--retry".into(),
        "2".into(),
        "--max-time".into(),
        "120".into(),
        "--max-filesize".into(),
        MAX_PAGE_BYTES.to_string(),
        "-o".into(),
        dest.to_string_lossy().into_owned(),
        "-w".into(),
        "%{http_code}".into(),
    ];
    for h in headers {
        args.push("-H".into());
        args.push(h.clone());
    }
    args.push(url.to_owned());
    args
}

/// Map the HTTP status; non-2xx responses leave no partial file behind.
fn classify(code: u16, dest: &Path) -> FetchStatus {
    let status = match code {
        200..=299 => return FetchStatus::Ok,
        404 => FetchStatus::NotFound,
        429 => FetchStatus::RateLimited,
        other => FetchStatus::Http(other),
    };
    let _ = std::fs::remove_file(dest);
    status
}

/// Markdown links out of an llms.txt-style index.
///
/// Resolved against the index URL and filtered to per-page `.md` targets
/// (query/fragment stripped). Non-markdown links are counted, not fetched
/// — llms-full.txt corpora and HTML pages are not page material.
#[must_use]
pub fn markdown_links(index_url: &str, text: &str) -> (Vec<String>, usize) {
    let mut pages = Vec::new();
    let mut skipped = 0;
    for target in link_targets(text) {
        let clean = target
            .split(['?', '#'])
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned();
        if clean.is_empty() {
            continue;
        }
        let Some(resolved) = resolve_url(index_url, &clean) else {
            skipped += 1;
            continue;
        };
        if super::is_md(&resolved) {
            if !pages.contains(&resolved) {
                pages.push(resolved);
            }
        } else {
            skipped += 1;
        }
    }
    (pages, skipped)
}

/// Every `](target)` markdown link target in order.
fn link_targets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(at) = text[i..].find("](") {
        let start = i + at + 2;
        let Some(len) = text[start..].find(')') else {
            break;
        };
        out.push(text[start..start + len].to_owned());
        i = start + len;
        if i >= bytes.len() {
            break;
        }
    }
    out
}

/// Resolve `target` against `base` (the index URL).
///
/// Absolute URLs pass through, `/rooted` paths join the host, and relative
/// paths join the index's directory with `.`/`..` normalization. Non-http
/// schemes return `None`.
#[must_use]
pub fn resolve_url(base: &str, target: &str) -> Option<String> {
    if target.starts_with("http://") || target.starts_with("https://") {
        return Some(target.to_owned());
    }
    if target.contains(':') {
        return None; // mailto:, ftp:, …
    }
    let scheme_end = base.find("://")? + 3;
    let host_end = base[scheme_end..]
        .find('/')
        .map_or(base.len(), |p| scheme_end + p);
    let origin = &base[..host_end];
    if let Some(rooted) = target.strip_prefix('/') {
        return Some(format!("{origin}/{rooted}"));
    }
    let base_path = &base[host_end..];
    let base_dir = base_path.rsplit_once('/').map_or("", |(d, _)| d);
    let mut segments: Vec<&str> = base_dir.split('/').filter(|s| !s.is_empty()).collect();
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            s => segments.push(s),
        }
    }
    Some(format!("{origin}/{}", segments.join("/")))
}

/// A cache file name for a page URL: the URL path with `/` flattened to
/// `-`, guaranteed to end in `.md`.
#[must_use]
pub fn page_slug(url: &str) -> String {
    let path = url.find("://").map_or(url, |at| {
        url[at + 3..].split_once('/').map_or("", |(_, p)| p)
    });
    let mut slug: String = path
        .trim_matches('/')
        .chars()
        .map(|c| match c {
            '/' => '-',
            c if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' => c,
            _ => '_',
        })
        .collect();
    if slug.is_empty() {
        "index".clone_into(&mut slug);
    }
    if !super::is_md(&slug) {
        slug.push_str(".md");
    }
    slug
}

/// The Context7 REST v2 request for a library id: URL + headers.
///
/// Keyless works at a low shared rate; `CONTEXT7_API_KEY` (env) adds a
/// Bearer header for the paid tier. Endpoint shape verified live
/// 2026-07-18: `GET /api/v2/context?libraryId=<id>&query=<q>&type=txt`
/// (the `query` parameter is required by the API).
#[must_use]
pub fn context7_request(
    library_id: &str,
    name: &str,
    api_key: Option<&str>,
) -> (String, Vec<String>) {
    let url = format!(
        "https://context7.com/api/v2/context?libraryId={}&query={}%20overview&type=txt",
        percent_encode(library_id),
        percent_encode(name),
    );
    let headers = api_key
        .map(|key| vec![format!("Authorization: Bearer {key}")])
        .unwrap_or_default();
    (url, headers)
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char);
            }
            other => {
                let _ = write!(out, "%{other:02X}");
            }
        }
    }
    out
}

/// Decompress a `.gz` file via the system `gzip -dc`.
///
/// docs.rs serves the rustdoc JSON gzipped at `/json.gz`; the plain
/// `/json` endpoint moved to zstd (observed live 2026-07-18) — gzip keeps
/// the toolchain universal.
///
/// # Errors
/// [`DocsError`] when gzip fails or the output cannot be written.
pub fn gunzip(gz: &Path, dest: &Path) -> Result<(), DocsError> {
    let output = Command::new("gzip")
        .arg("-dc")
        .arg(gz)
        .output()
        .map_err(|source| DocsError::CurlSpawn {
            url: gz.to_string_lossy().into_owned(),
            source,
        })?;
    if !output.status.success() {
        return Err(DocsError::CurlFailed {
            url: gz.to_string_lossy().into_owned(),
            detail: format!(
                "gzip -dc failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    std::fs::write(dest, &output.stdout).map_err(|source| DocsError::Io {
        path: dest.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Provenance: first lines of https://hono.dev/llms.txt as captured
    // 2026-07-18 (absolute links, none per-page .md) plus the cucumber-rs
    // book SUMMARY.md shape (relative .md links), same capture date.
    const HONO_SNIPPET: &str = "# Hono\n\n## Docs\n\n- [Full Docs](https://hono.dev/llms-full.txt) Full documentation.\n- [Index](https://hono.dev/docs/index)\n";
    const SUMMARY_SNIPPET: &str = "# Summary\n\n- [Introduction](introduction.md)\n- [Writing tests](writing/index.md)\n    - [Capturing](writing/capturing.md)\n";

    #[test]
    fn llms_txt_absolute_non_md_links_are_skipped_not_fetched() {
        let (pages, skipped) = markdown_links("https://hono.dev/llms.txt", HONO_SNIPPET);
        assert!(pages.is_empty(), "{pages:?}");
        assert_eq!(skipped, 2, "llms-full.txt and the HTML page");
    }

    #[test]
    fn summary_md_relative_links_resolve_against_the_index() {
        let base =
            "https://raw.githubusercontent.com/cucumber-rs/cucumber/main/book/src/SUMMARY.md";
        let (pages, skipped) = markdown_links(base, SUMMARY_SNIPPET);
        assert_eq!(skipped, 0);
        assert_eq!(
            pages,
            vec![
                "https://raw.githubusercontent.com/cucumber-rs/cucumber/main/book/src/introduction.md",
                "https://raw.githubusercontent.com/cucumber-rs/cucumber/main/book/src/writing/index.md",
                "https://raw.githubusercontent.com/cucumber-rs/cucumber/main/book/src/writing/capturing.md",
            ]
        );
    }

    #[test]
    fn url_resolution_handles_rooted_dotted_and_foreign_schemes() {
        let base = "https://example.dev/docs/guide/llms.txt";
        assert_eq!(
            resolve_url(base, "/api/intro.md").as_deref(),
            Some("https://example.dev/api/intro.md")
        );
        assert_eq!(
            resolve_url(base, "../other/page.md").as_deref(),
            Some("https://example.dev/docs/other/page.md")
        );
        assert_eq!(
            resolve_url(base, "./sib.md").as_deref(),
            Some("https://example.dev/docs/guide/sib.md")
        );
        assert_eq!(resolve_url(base, "mailto:x@y.z"), None);
    }

    #[test]
    fn query_and_fragment_are_stripped_before_the_md_check() {
        let (pages, _) = markdown_links(
            "https://example.dev/llms.txt",
            "[a](https://example.dev/p.md?ref=1) [b](https://example.dev/q.md#top)",
        );
        assert_eq!(
            pages,
            vec!["https://example.dev/p.md", "https://example.dev/q.md"]
        );
    }

    #[test]
    fn page_slugs_flatten_paths_and_end_in_md() {
        assert_eq!(
            page_slug("https://x.dev/docs/middleware/cors.md"),
            "docs-middleware-cors.md"
        );
        assert_eq!(page_slug("https://x.dev/"), "index.md");
        assert_eq!(page_slug("https://x.dev/docs/index"), "docs-index.md");
    }

    #[test]
    fn context7_request_shape_and_key_pickup() {
        let (url, headers) = context7_request("/websites/hono_dev", "hono", None);
        assert_eq!(
            url,
            "https://context7.com/api/v2/context?libraryId=/websites/hono_dev&query=hono%20overview&type=txt"
        );
        assert!(headers.is_empty(), "keyless by default");
        let (_, headers) = context7_request("/websites/hono_dev", "hono", Some("k-123"));
        assert_eq!(headers, vec!["Authorization: Bearer k-123".to_owned()]);
    }
}

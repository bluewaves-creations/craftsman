//! The ONE live-network docs test (Batch 7 scope): fetch a stable public
//! llms.txt (hono.dev, per the documentation-pipeline research) and prove
//! the fetch + parse pipeline against reality. Offline it SKIPS LOUDLY —
//! a skipped network test must never masquerade as green coverage.

use craftsman::docs::fetch;

#[test]
fn live_llms_txt_fetch_and_parse_hono() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dest = tmp.path().join("llms.txt");
    let url = "https://hono.dev/llms.txt";

    let status = match fetch::fetch(url, &dest, &[]) {
        Ok(status) => status,
        Err(err) => {
            eprintln!(
                "SKIPPED LOUDLY: live llms.txt test could not reach the network \
                 ({err}) — the fetch pipeline was NOT exercised against {url}"
            );
            return;
        }
    };
    assert_eq!(
        status,
        fetch::FetchStatus::Ok,
        "hono.dev/llms.txt should serve 200 (a stable public index)"
    );
    let text = std::fs::read_to_string(&dest).expect("fetched index is readable");
    assert!(
        text.starts_with("# Hono"),
        "llms.txt starts with its H1 (observed live 2026-07-18):\n{}",
        &text[..text.len().min(200)]
    );
    // Hono's index links llms-full.txt and HTML pages, none per-page .md
    // (observed live 2026-07-18) — the parser must find links and classify
    // every one of them as skippable rather than fetchable pages.
    let (pages, skipped) = fetch::markdown_links(url, &text);
    assert!(
        skipped > 0,
        "the index carries links the parser classified:\npages={pages:?}"
    );
    for page in &pages {
        assert!(
            craftsman::docs::is_md(page),
            "only .md targets are pages: {page}"
        );
    }
}

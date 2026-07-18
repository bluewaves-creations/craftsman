//! Step definitions — the recovered docs scenarios (Batch 11): sync
//! refusals, the offline file source, zero-hit search, unknown-page
//! listing, version promotion, and the live llms-txt sync.

use cucumber::{given, then, when};

use crate::{CliWorld, MINIMAL_CONFIG};

fn bare_project(w: &mut CliWorld) {
    w.write("craftsman.toml", MINIMAL_CONFIG);
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
}

#[given(expr = "a craftsman project with no docs source named {string}")]
fn project_without_named_source(w: &mut CliWorld, name: String) {
    bare_project(w);
    assert!(
        !name.is_empty(),
        "the scenario names the library it expects to be undeclared"
    );
}

#[given("a craftsman project with no docs sources declared")]
fn project_without_sources(w: &mut CliWorld) {
    bare_project(w);
}

#[given("a craftsman project with a file docs source pointing at a local markdown directory")]
fn project_with_file_source(w: &mut CliWorld) {
    bare_project(w);
    std::fs::create_dir_all(w.project_dir().join("docs-src")).expect("mkdirs");
    w.write("docs-src/guide.md", "# Guide\n\nLocal truth lives here.\n");
    w.run_craftsman(&[
        "docs", "add", "local", "--source", "file", "--path", "docs-src",
    ]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "priming docs add must pass:\n{}",
        w.combined_output()
    );
}

#[when("the source is synced and then searched for its content")]
fn sync_then_search(w: &mut CliWorld) {
    w.run_craftsman(&["docs", "sync"]);
    w.prev_exit = w.output().status.code();
    w.run_craftsman(&["docs", "search", "Local truth"]);
}

#[then("both commands exit 0")]
fn both_commands_exit_0(w: &mut CliWorld) {
    assert_eq!(w.prev_exit, Some(0), "the first command did not exit 0");
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "the second command did not exit 0:\n{}",
        w.combined_output()
    );
}

#[then("the search names the local page")]
fn search_names_local_page(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("guide.md"),
        "search does not name guide.md:\n{combined}"
    );
}

/// A seeded cache (no sync ran): manifest + one or two cached pages.
fn seed_cache(w: &mut CliWorld, lib: &str, pages: &[(&str, &str)]) {
    bare_project(w);
    let pages_dir = w
        .project_dir()
        .join(".craftsman/docs")
        .join(format!("{lib}@1.0.0/pages"));
    std::fs::create_dir_all(&pages_dir).expect("mkdirs");
    for (name, content) in pages {
        std::fs::write(pages_dir.join(name), content).expect("write page");
    }
    let manifest = format!(
        "{{\n  \"libraries\": {{\n    \"{lib}\": {{\n      \"source\": \"llms-txt\",\n      \
         \"urls\": [\"https://example.dev/llms.txt\"],\n      \"version\": \"1.0.0\"\n    }}\n  }}\n}}\n"
    );
    w.write(".craftsman/docs/manifest.json", &manifest);
}

#[given("a craftsman project with a synced docs cache")]
fn project_with_synced_cache(w: &mut CliWorld) {
    seed_cache(w, "demo", &[("intro.md", "# Intro\n\nStreaming here.\n")]);
}

#[given(
    expr = "a craftsman project with a seeded docs cache for library {string} holding pages intro.md and faq.md"
)]
fn project_with_two_page_cache(w: &mut CliWorld, lib: String) {
    seed_cache(
        w,
        &lib,
        &[
            ("intro.md", "# Intro\n\nOpening words.\n"),
            ("faq.md", "# FAQ\n\nAnswers.\n"),
        ],
    );
}

#[then("the output names the pages that do exist")]
fn output_names_existing_pages(w: &mut CliWorld) {
    let combined = w.combined_output();
    for page in ["intro.md", "faq.md"] {
        assert!(
            combined.contains(page),
            "unknown-page error does not list {page}:\n{combined}"
        );
    }
}

/// The manifest for the version-promotion scenario: a file source whose
/// pin the test moves between syncs (lockfiles win, but there are none).
fn file_source_manifest(pin: &str) -> String {
    format!(
        "{{\n  \"libraries\": {{\n    \"demo\": {{\n      \"source\": \"file\",\n      \
         \"urls\": [],\n      \"path\": \"docs-src\",\n      \"pin\": \"{pin}\"\n    }}\n  }}\n}}\n"
    )
}

#[given("a docs cache holding library \"demo\" at version 1.0.0")]
fn cache_at_version_1(w: &mut CliWorld) {
    bare_project(w);
    std::fs::create_dir_all(w.project_dir().join("docs-src")).expect("mkdirs");
    w.write("docs-src/guide.md", "# Guide\n\nVersioned truth.\n");
    std::fs::create_dir_all(w.project_dir().join(".craftsman/docs")).expect("mkdirs");
    w.write(
        ".craftsman/docs/manifest.json",
        &file_source_manifest("1.0.0"),
    );
    w.run_craftsman(&["docs", "sync", "demo"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "priming sync must pass:\n{}",
        w.combined_output()
    );
    assert!(
        w.project_dir().join(".craftsman/docs/demo@1.0.0").is_dir(),
        "the 1.0.0 cache must exist after the priming sync"
    );
}

#[when("version 2.0.0 of \"demo\" is synced")]
fn sync_version_2(w: &mut CliWorld) {
    w.write(
        ".craftsman/docs/manifest.json",
        &file_source_manifest("2.0.0"),
    );
    w.run_craftsman(&["docs", "sync", "demo"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "sync of 2.0.0 must pass:\n{}",
        w.combined_output()
    );
}

#[then("the cache holds version 2.0.0")]
fn cache_holds_version_2(w: &mut CliWorld) {
    let dir = w.project_dir().join(".craftsman/docs/demo@2.0.0");
    assert!(dir.is_dir(), "{} missing", dir.display());
}

#[then("the 1.0.0 copy is gone")]
fn old_copy_gone(w: &mut CliWorld) {
    let dir = w.project_dir().join(".craftsman/docs/demo@1.0.0");
    assert!(!dir.exists(), "{} survived the promotion", dir.display());
}

/// A live llms-txt-style index whose links really are per-page `.md`
/// files — the cucumber-rs book's SUMMARY.md, the same source this
/// repository's own docs table declares. (hono.dev/llms.txt, the other
/// stable index, lists no `.md` pages — see `docs_live.rs`.)
const LIVE_LLMS_INDEX: &str =
    "https://raw.githubusercontent.com/cucumber-rs/cucumber/main/book/src/SUMMARY.md";

#[given("a craftsman project with an llms-txt docs source for a live library")]
fn project_with_live_llms_source(w: &mut CliWorld) {
    bare_project(w);
    w.run_craftsman(&[
        "docs",
        "add",
        "cucumber-book",
        "--source",
        "llms-txt",
        "--url",
        LIVE_LLMS_INDEX,
    ]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "priming docs add must pass:\n{}",
        w.combined_output()
    );
}

#[when("I run craftsman docs sync for that library")]
fn run_docs_sync_live(w: &mut CliWorld) {
    w.run_craftsman(&["docs", "sync", "cucumber-book"]);
}

#[then("the cached pages are markdown files searchable offline")]
fn cached_pages_searchable(w: &mut CliWorld) {
    let docs = w.project_dir().join(".craftsman/docs");
    let lib = std::fs::read_dir(&docs)
        .expect("read docs cache")
        .filter_map(Result::ok)
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("cucumber-book@")
        })
        .expect("a cucumber-book@<version> cache dir");
    let pages = lib.path().join("pages");
    let md = std::fs::read_dir(&pages)
        .expect("read pages")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .count();
    assert!(md > 0, "no markdown pages under {}", pages.display());
    w.run_craftsman(&["docs", "search", "cucumber"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "offline search must pass:\n{}",
        w.combined_output()
    );
}

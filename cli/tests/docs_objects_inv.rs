//! GAP-R08 root-cause test: `docs get` on an objects-inv library resolves
//! an uncached object via the synced inventory, fetches the target page on
//! demand, and caches it — the pipeline's one documented network
//! exception, proven hermetically against a `file://` site (the second
//! get must survive the source's deletion).

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output};

fn craftsman(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_craftsman"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn craftsman")
}

fn assert_ok(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    stdout
}

/// A local sphinx-style site: a real zlib `objects.inv` plus the one page
/// it indexes.
fn build_site() -> std::path::PathBuf {
    let site_dir = std::env::temp_dir().join("craftsman-r08-site");
    let _ = std::fs::remove_dir_all(&site_dir);
    std::fs::create_dir_all(&site_dir).expect("site dir");
    let header = b"# Sphinx inventory version 2\n# Project: mylib\n# Version: 1.0\n# The remainder of this file is compressed using zlib.\n";
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(b"mylib.core py:module 1 core.html -\n")
        .expect("compress");
    let payload = encoder.finish().expect("zlib");
    let mut inv = header.to_vec();
    inv.extend(payload);
    std::fs::write(site_dir.join("objects.inv"), inv).expect("inventory");
    std::fs::write(
        site_dir.join("core.html"),
        "<html><body><h1>Core</h1><p>The core module holds the truth.</p></body></html>",
    )
    .expect("page");
    site_dir
}

#[test]
fn docs_get_objects_inv_fetches_on_demand_then_serves_from_cache() {
    let site_dir = build_site();
    let proj = tempfile::tempdir().expect("project");
    std::fs::write(
        proj.path().join("craftsman.toml"),
        "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n",
    )
    .expect("config");

    let url = format!("file://{}/objects.inv", site_dir.display());
    assert_ok(&craftsman(
        proj.path(),
        &[
            "docs",
            "add",
            "mylib",
            "--source",
            "objects-inv",
            "--url",
            &url,
        ],
    ));
    assert_ok(&craftsman(proj.path(), &["docs", "sync"]));

    // First get: uncached — resolved via the inventory, fetched, cached.
    let stdout = assert_ok(&craftsman(
        proj.path(),
        &["docs", "get", "mylib/mylib.core"],
    ));
    assert!(
        stdout.contains("The core module holds the truth"),
        "the fetched page must print:\n{stdout}"
    );

    // Second get: the source is gone; only the cache can answer.
    std::fs::remove_file(site_dir.join("core.html")).expect("delete source");
    let stdout = assert_ok(&craftsman(
        proj.path(),
        &["docs", "get", "mylib/mylib.core"],
    ));
    assert!(
        stdout.contains("The core module holds the truth"),
        "the second get must be served from the cache:\n{stdout}"
    );
}

//! Contract-sweep completion (Batch 9b): the `--json` happy paths that
//! need pre-resolved hermetic tools or a local docs source — split from
//! `contract.rs` to keep both files inside the health gate's file budget.
//!
//! `security` and `mutate` run offline against `~/.craftsman/tools` and
//! skip LOUDLY when the pinned tools were never resolved (the sweep must
//! never download); `docs sync` runs against a `file` source and needs no
//! network at all.

mod util;

use util::{assert_json, combined, craftsman, fixture_project};

/// The hermetic tools dir (`~/.craftsman/tools`), if it exists.
fn tools_dir() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    let dir = std::path::PathBuf::from(home).join(".craftsman/tools");
    dir.is_dir().then_some(dir)
}

/// `security --json` offline against pre-resolved tools: gitleaks +
/// osv-scanner binaries, the osv offline databases, and the semgrep
/// ruleset must already sit in `~/.craftsman/tools` at the default pins —
/// otherwise the sweep would download, so it skips LOUDLY instead.
#[test]
fn security_json_happy_path_offline() {
    let gitleaks = craftsman::gates::adapter::tool("gitleaks").expect("in table");
    let osv = craftsman::gates::adapter::tool("osv-scanner").expect("in table");
    let semgrep = craftsman::gates::adapter::tool("semgrep").expect("in table");
    let Some(tools) = tools_dir() else {
        eprintln!("SKIPPED (loudly): no ~/.craftsman/tools — security tools unresolved");
        return;
    };
    let needed = [
        format!("gitleaks@{}", gitleaks.default_version),
        format!("osv-scanner@{}", osv.default_version),
        format!("semgrep-rules@{}", semgrep.default_version),
        "osv-db".to_owned(),
    ];
    for entry in &needed {
        if !tools.join(entry).exists() {
            eprintln!(
                "SKIPPED (loudly): {} not pre-resolved in {} — run \
                 `craftsman security` once (network) to enable this sweep",
                entry,
                tools.display()
            );
            return;
        }
    }
    let tmp = fixture_project();
    assert_json(tmp.path(), &["security", "--json"], None, &[0, 1]);
}

/// `mutate --json` on the tiny rust fixture with a clean tree: the
/// diff-scoped run reports "nothing to mutate" and exits 0 — the command
/// path and JSON contract without a mutation run (score paths are proven
/// live in tests/mutate.rs). Skips loudly when cargo-mutants was never
/// hermetically installed (resolution would `cargo install` from the
/// network).
#[test]
fn mutate_json_happy_path_offline() {
    let Some(tools) = tools_dir() else {
        eprintln!("SKIPPED (loudly): no ~/.craftsman/tools — cargo-mutants unresolved");
        return;
    };
    let has_mutants = std::fs::read_dir(&tools).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("cargo-mutants@"))
                && e.path().join("bin/cargo-mutants").is_file()
        })
    });
    if !has_mutants {
        eprintln!(
            "SKIPPED (loudly): no cargo-mutants@* under {} — run \
             `craftsman mutate` once (cargo install) to enable this sweep",
            tools.display()
        );
        return;
    }
    let tmp = fixture_project();
    let out = craftsman(tmp.path(), &["mutate", "--json"], None);
    assert_eq!(
        out.status.code(),
        Some(0),
        "clean tree: nothing to mutate is a pass:\n{}",
        combined(&out)
    );
    let doc: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&out.stdout))
        .expect("mutate --json emits JSON");
    assert_eq!(doc["gate"], "mutate");
    assert!(
        doc["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|n| n.as_str().unwrap_or_default().contains("nothing to mutate")),
        "{doc:#}"
    );
}

/// `docs sync --json` fully offline: a `file` source pointing at a local
/// markdown file needs no network at all.
#[test]
fn docs_sync_json_happy_path_offline_file_source() {
    let tmp = fixture_project();
    let dir = tmp.path();
    std::fs::create_dir_all(dir.join("docs-src")).expect("mkdirs");
    std::fs::write(
        dir.join("docs-src/guide.md"),
        "# Guide\n\nEntirely local material.\n",
    )
    .expect("write local doc");
    let add = craftsman(
        dir,
        &[
            "docs",
            "add",
            "localdocs",
            "--source",
            "file",
            "--path",
            "docs-src",
        ],
        None,
    );
    assert_eq!(add.status.code(), Some(0), "{}", combined(&add));
    assert_json(dir, &["docs", "sync", "localdocs", "--json"], None, &[0]);
    // The synced page is immediately searchable offline.
    let search = craftsman(
        dir,
        &["docs", "search", "local material", "--lib", "localdocs"],
        None,
    );
    assert_eq!(search.status.code(), Some(0), "{}", combined(&search));
    assert!(
        combined(&search).contains("guide.md"),
        "{}",
        combined(&search)
    );
}

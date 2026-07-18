//! GAP-R07 pin: `adr stale` derives its staleness verdict from git
//! history — an active ADR is flagged once commits touch its cited files
//! after the ADR's own last commit, and not before.

use std::path::Path;
use std::process::Command;

use craftsman::adr;
use craftsman::config::Config;

fn git(dir: &Path, args: &[&str]) {
    let base = [
        "-c",
        "user.name=fixture",
        "-c",
        "user.email=f@example.invalid",
    ];
    let full: Vec<&str> = base.into_iter().chain(args.iter().copied()).collect();
    let status = Command::new("git")
        .args(&full)
        .current_dir(dir)
        .status()
        .expect("spawn git");
    assert!(status.success(), "git {args:?}");
}

#[test]
fn adr_stale_flags_from_history_and_only_after_cited_files_move() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).expect("mkdirs");
    std::fs::create_dir_all(root.join("decisions")).expect("mkdirs");
    std::fs::write(
        root.join("craftsman.toml"),
        "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[adr]\nstale-commits = 0\n",
    )
    .expect("config");
    std::fs::write(root.join("src/core.rs"), "pub fn core() {}\n").expect("source");
    std::fs::write(
        root.join("decisions/ADR-001-core-shape.md"),
        "# ADR-001: Core shape\n\nStatus: accepted · Date: 2026-07-18\n\n\
         The core lives in src/core.rs and stays synchronous.\n",
    )
    .expect("adr");
    git(root, &["init", "--quiet"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "--quiet", "-m", "adopt ADR-001"]);

    let loaded = Config::load(root).expect("config loads");

    let fresh = adr::stale(&loaded.root, &loaded.config).expect("stale runs");
    assert!(
        fresh.is_empty(),
        "no cited file moved yet — nothing is stale: {fresh:?}"
    );

    std::fs::write(root.join("src/core.rs"), "pub fn core() { /* moved */ }\n").expect("edit");
    git(root, &["add", "-A"]);
    git(root, &["commit", "--quiet", "-m", "core moves on"]);

    let findings = adr::stale(&loaded.root, &loaded.config).expect("stale runs");
    assert_eq!(findings.len(), 1, "{findings:?}");
    let f = &findings[0];
    assert_eq!(f.file, "decisions/ADR-001-core-shape.md");
    assert_eq!(f.commits_since, 1, "{f:?}");
    assert!(
        f.cited.iter().any(|c| c == "src/core.rs"),
        "the verdict names the cited file: {f:?}"
    );
    assert!(f.advice.contains("confirm or supersede"), "{f:?}");
}

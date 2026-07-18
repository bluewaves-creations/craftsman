//! GAP-R04 root-cause test: `lint --changed` narrows findings to the files
//! changed against HEAD. Two badly formatted files, one declared changed —
//! the outcome must carry exactly that file's finding, root-relative.
//! (The pin exposed a defect: cargo fmt reports absolute paths, so the
//! changed-set retain dropped every fmt finding — a formatting violation
//! in a changed file sailed through `lint --changed` as clean.)

use craftsman::config::Config;
use craftsman::gates::lint;

#[test]
fn lint_changed_narrows_to_the_changed_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    std::fs::create_dir_all(dir.join("src")).expect("mkdirs");
    let write = |rel: &str, content: &str| {
        std::fs::write(dir.join(rel), content).expect("write");
    };
    write(
        "Cargo.toml",
        "[package]\nname = \"lintfix\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    write("src/lib.rs", "pub mod a;\npub mod b;\n");
    write("src/a.rs", "pub fn a( x:i32)->i32{ x+1 }\n");
    write("src/b.rs", "pub fn b( x:i32)->i32{ x+2 }\n");
    write(
        "craftsman.toml",
        "[project]\nname = \"lintfix\"\nstacks = [\"rust\"]\n\n[gates]\nlint = \"strict\"\n",
    );

    let loaded = Config::load(dir).expect("config loads");
    let changed = vec!["src/a.rs".to_owned()];
    let outcome = lint::run(
        &loaded.root,
        &loaded.config,
        Some(&changed),
        craftsman::config::GateMode::Strict,
    )
    .expect("lint runs");

    assert!(
        !outcome.findings.is_empty(),
        "the changed file's finding must surface"
    );
    assert!(
        outcome.findings.iter().all(|f| f.file == "src/a.rs"),
        "findings must be narrowed to the changed set, got: {:?}",
        outcome
            .findings
            .iter()
            .map(|f| f.file.as_str())
            .collect::<Vec<_>>()
    );
}

//! Step definitions — repository, bootstrap, docs, session, and the
//! generic run/exit/output assertion steps.

use std::process::Command;

use cucumber::{given, then, when};

use crate::{CliWorld, MINIMAL_CONFIG};

#[given("an empty project directory")]
fn empty_project_directory(w: &mut CliWorld) {
    let _ = w.project_dir();
}

#[given("any directory")]
fn any_directory(w: &mut CliWorld) {
    let _ = w.project_dir();
}

#[given("an empty directory that is not a git repository")]
fn non_git_directory(w: &mut CliWorld) {
    let _ = w.project_dir();
}

#[then(expr = "the file {word} exists")]
fn file_exists(w: &mut CliWorld, rel: String) {
    let path = w.project_dir().join(&rel);
    assert!(path.is_file(), "{} does not exist", path.display());
}

/// `git init` + `git add -A` in the fixture dir (arch and health census
/// tracked files via `git ls-files`; no commit needed).
fn git_init_add(dir: &std::path::Path) {
    for args in [&["init", "--quiet"][..], &["add", "-A"][..]] {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }
}

/// Recursive fixture copy, skipping caches and per-run state (`.git`,
/// `.craftsman`, `.venv`, `node_modules`, `__pycache__`, `target`).
pub fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("mkdirs");
    for entry in std::fs::read_dir(from).expect("read fixture dir") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name();
        let skip = [
            ".git",
            ".craftsman",
            ".venv",
            "node_modules",
            "__pycache__",
            "target",
        ];
        if skip.iter().any(|s| name.to_string_lossy() == *s) {
            continue;
        }
        let src = entry.path();
        let dest = to.join(&name);
        if src.is_dir() {
            copy_tree(&src, &dest);
        } else {
            std::fs::copy(&src, &dest).unwrap_or_else(|e| panic!("copy {}: {e}", src.display()));
        }
    }
}

/// Fresh single-commit repository: init, stage everything, commit — for
/// fixtures that need a resolvable `HEAD`. The identity is written into
/// the repo config (not passed per-command) so commits the CLI itself
/// makes later also resolve it — CI runners have no global identity.
pub fn git_init_commit_all(dir: &std::path::Path) {
    git_init_add(dir);
    for args in [
        &["config", "user.name", "fixture"][..],
        &["config", "user.email", "fixture@example.invalid"][..],
        &["commit", "--quiet", "-m", "init"][..],
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }
}

#[given("a craftsman project with an arch deny rule and a violating import")]
fn arch_violation_fixture(w: &mut CliWorld) {
    w.write(
        "craftsman.toml",
        "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\narch = \"strict\"\n\n[arch]\ndeny = [\"src/a -> src/b\"]\n",
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("src/a")).expect("mkdirs");
    std::fs::create_dir_all(dir.join("src/b")).expect("mkdirs");
    w.write("src/a/mod.rs", "use crate::b::helper;\n");
    w.write("src/b/mod.rs", "pub fn helper() {}\n");
    git_init_add(&dir);
}

#[given("a craftsman project whose source has a function longer than the health limit")]
fn health_long_function_fixture(w: &mut CliWorld) {
    w.write(
        "craftsman.toml",
        "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\nhealth = \"strict\"\n\n[health]\nmax-function-lines = 5\n",
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("src")).expect("mkdirs");
    w.write(
        "src/lib.rs",
        "pub fn sprawling() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    let d = 4;\n    let e = 5;\n    let f = 6;\n    let _ = a + b + c + d + e + f;\n}\n",
    );
    git_init_add(&dir);
}

#[given(expr = "a craftsman project with a seeded docs cache for library {string}")]
fn seeded_docs_cache(w: &mut CliWorld, lib: String) {
    w.write("craftsman.toml", MINIMAL_CONFIG);
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    let dir = w.project_dir();
    let cache = dir.join(".craftsman/docs");
    let pages = cache.join(format!("{lib}@1.0.0/pages"));
    std::fs::create_dir_all(&pages).expect("mkdirs");
    std::fs::write(
        pages.join("intro.md"),
        "# Intro\n\nStreaming responses are the core feature.\n",
    )
    .expect("write page");
    let manifest = format!(
        "{{\n  \"libraries\": {{\n    \"{lib}\": {{\n      \"source\": \"llms-txt\",\n      \
         \"urls\": [\"https://example.dev/llms.txt\"],\n      \"version\": \"1.0.0\"\n    }}\n  }}\n}}\n"
    );
    std::fs::write(cache.join("manifest.json"), manifest).expect("write manifest");
}

#[given(expr = "a batch 7 extract recorded the decision {string}")]
fn batch_extract_recorded(w: &mut CliWorld, decision: String) {
    w.run_craftsman(&["extract", "--batch", "7", "--decision", &decision]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "priming extract must pass:\n{}",
        w.combined_output()
    );
}

#[given(expr = "a craftsman project with decisions {string} and {string}")]
fn project_with_decisions(w: &mut CliWorld, first: String, second: String) {
    w.write("craftsman.toml", MINIMAL_CONFIG);
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("decisions")).expect("mkdirs");
    for (file, title) in [("ADR-001-first.md", &first), ("ADR-002-second.md", &second)] {
        w.write(
            &format!("decisions/{file}"),
            &format!("# {title}\n\nStatus: accepted · Date: 2026-07-18\n\nBody.\n"),
        );
    }
}

#[then(expr = "the decisions index lists {string}")]
fn decisions_index_lists(w: &mut CliWorld, title: String) {
    let path = w.project_dir().join("decisions/index.md");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(
        text.lines().any(|l| l.contains(&title)),
        "decisions/index.md has no line for {title:?}:\n{text}"
    );
}

#[given("an empty git repository directory")]
fn empty_git_repository_directory(w: &mut CliWorld) {
    let dir = w.project_dir();
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&dir)
        .status()
        .expect("spawn git init");
    assert!(status.success(), "git init failed in {}", dir.display());
}

#[given("craftsman init has already scaffolded it")]
fn init_already_scaffolded(w: &mut CliWorld) {
    w.run_craftsman(&["init", "--name", "demo", "--stack", "rust"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "priming init must pass:\n{}",
        w.combined_output()
    );
}

#[given("a sandboxed home directory with a Claude Code marker")]
fn sandboxed_home(w: &mut CliWorld) {
    let home = tempfile::tempdir().expect("home tempdir");
    std::fs::create_dir_all(home.path().join(".claude")).expect("claude marker");
    w.home = Some(home);
    // Setup needs no project; give the world a plain directory to run in.
    let _ = w.project_dir();
}

#[when("I run craftsman setup against the sandboxed home")]
fn run_setup_sandboxed(w: &mut CliWorld) {
    assert!(w.home.is_some(), "a sandboxed home must be prepared first");
    w.run_craftsman(&["setup"]);
}

#[then(expr = "the sandboxed home holds the canonical skill {string} with a sentinel")]
fn sandboxed_home_holds_skill(w: &mut CliWorld, skill: String) {
    let home = w.home.as_ref().expect("sandboxed home").path();
    let dir = home.join(".agents/skills").join(&skill);
    assert!(
        dir.join("SKILL.md").is_file(),
        "{} missing SKILL.md",
        dir.display()
    );
    let sentinel = std::fs::read_to_string(dir.join(".craftsman-setup"))
        .unwrap_or_else(|e| panic!("sentinel in {}: {e}", dir.display()));
    let hash = sentinel
        .lines()
        .nth(1)
        .expect("sentinel line 2 is the tree sha256");
    assert_eq!(hash.len(), 64, "expected a sha256, got {hash:?}");
}

#[then(expr = "the sandboxed home serves {string} to Claude Code via a symlink")]
fn sandboxed_home_serves_skill(w: &mut CliWorld, skill: String) {
    let home = w.home.as_ref().expect("sandboxed home").path();
    let link = home.join(".claude/skills").join(&skill);
    let target = std::fs::read_link(&link)
        .unwrap_or_else(|e| panic!("{} must be a symlink: {e}", link.display()));
    assert!(
        target.starts_with(home.join(".agents/skills")),
        "{} must resolve into the canonical dir, points to {}",
        link.display(),
        target.display()
    );
}

#[given("the project is a fresh git repository")]
fn fresh_git_repository(w: &mut CliWorld) {
    let dir = w.project_dir();
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&dir)
        .status()
        .expect("spawn git init");
    assert!(status.success(), "git init failed in {}", dir.display());
}

#[when(expr = "I run craftsman with {string}")]
fn run_with_args(w: &mut CliWorld, args: String) {
    let args: Vec<&str> = args.split_whitespace().collect();
    w.run_craftsman(&args);
}

#[when(expr = "I run craftsman verify for the scenario {string}")]
fn run_verify_scenario(w: &mut CliWorld, name: String) {
    w.run_craftsman(&["verify", "--scenario", &name]);
}

#[then(expr = "the exit code is {int}")]
fn exit_code_is(w: &mut CliWorld, expected: i32) {
    let actual = w.output().status.code();
    assert_eq!(
        actual,
        Some(expected),
        "expected exit {expected}, got {actual:?}; output:\n{}",
        w.combined_output()
    );
}

#[then(expr = "the output contains {string}")]
fn output_contains(w: &mut CliWorld, needle: String) {
    let combined = w.combined_output();
    assert!(
        combined.contains(&needle),
        "output does not contain {needle:?}:\n{combined}"
    );
}

#[then(expr = "stdout is valid JSON listing {int} scenarios")]
fn stdout_is_json_with_scenarios(w: &mut CliWorld, count: usize) {
    let stdout = String::from_utf8_lossy(&w.output().stdout);
    let doc: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{stdout}"));
    let scenarios = doc["scenarios"].as_array().expect("a `scenarios` array");
    assert_eq!(scenarios.len(), count, "in:\n{stdout}");
}

/// A verify-green fixture whose repository has `HEAD` unborn: fresh
/// `git init`, everything staged, no commit yet (ledger finding 6).
#[given("a green craftsman project whose repository has no commits yet")]
fn green_project_with_unborn_head(w: &mut CliWorld) {
    crate::project_steps::scaffold_green_fixture(w, "craftsman-spec-first-commit-fixture");
    let dir = w.project_dir();
    let _ = std::fs::remove_dir_all(dir.join(".git"));
    std::fs::write(dir.join(".gitignore"), "target/\n.craftsman/\nCargo.lock\n")
        .expect("write .gitignore");
    for args in [
        &["init", "--quiet"][..],
        &["config", "user.name", "fixture"][..],
        &["config", "user.email", "fixture@example.invalid"][..],
        &["add", "-A"][..],
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }
}

#[when("I run craftsman commit for the staged tree")]
fn run_commit_for_staged_tree(w: &mut CliWorld) {
    w.run_craftsman(&[
        "commit",
        "--type",
        "chore",
        "--message",
        "bring the tree under craftsman",
    ]);
}

#[then("the repository's only commit carries a Verified-by trailer")]
fn only_commit_carries_verified_by(w: &mut CliWorld) {
    let dir = w.project_dir();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("spawn git");
        assert!(out.status.success(), "git {args:?} failed");
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    assert_eq!(
        git(&["rev-list", "--count", "HEAD"]).trim(),
        "1",
        "expected exactly one commit"
    );
    let body = git(&["log", "-1", "--format=%B"]);
    assert!(
        body.contains("Verified-by:"),
        "first commit must carry the CLI-written trailer:\n{body}"
    );
}

#[then(expr = "the scaffold includes {string}")]
fn scaffold_includes(w: &mut CliWorld, rel: String) {
    let path = w.project_dir().join(&rel);
    assert!(path.is_file(), "{} was not scaffolded", path.display());
}

#[then(expr = "the configured spec path ends with {string}")]
fn configured_spec_path_ends_with(w: &mut CliWorld, suffix: String) {
    let config = std::fs::read_to_string(w.project_dir().join("craftsman.toml"))
        .expect("scaffolded craftsman.toml");
    let spec = config
        .lines()
        .find_map(|l| l.trim().strip_prefix("spec = "))
        .expect("config declares a spec path")
        .trim_matches('"')
        .to_owned();
    assert!(
        spec.ends_with(&suffix),
        "configured spec {spec:?} does not end with {suffix:?}"
    );
}

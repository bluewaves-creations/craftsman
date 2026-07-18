//! Shared helpers for the contract-sweep test targets (`contract.rs`,
//! `contract_offline.rs`): binary invocation, a committed offline fixture
//! project, and the JSON-contract assertion.

use std::path::Path;
use std::process::{Command, Output};

pub fn craftsman(dir: &Path, args: &[&str], home: Option<&Path>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_craftsman"));
    cmd.args(args).current_dir(dir);
    if let Some(home) = home {
        cmd.env("HOME", home);
    }
    cmd.output().expect("spawn craftsman")
}

pub fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A committed fixture project every offline happy path can run against.
pub fn fixture_project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let write = |rel: &str, content: &str| {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdirs");
        }
        std::fs::write(&path, content).unwrap_or_else(|e| panic!("write {rel}: {e}"));
    };
    write(
        "craftsman.toml",
        "[project]\nname = \"contract\"\nstacks = [\"rust\"]\n\n[gates]\nverify = \"strict\"\n\n[arch]\ndeny = [\"src/a -> src/b\"]\n",
    );
    write(
        "SPEC.md",
        "Feature: Contract fixture\n\n  Scenario: First behavior\n\n  Scenario: Second behavior\n",
    );
    write(
        "PLAN.md",
        "# Plan\n\n## Batch 1\n\nScenarios:\n- First behavior\n- Second behavior\n",
    );
    write(
        "decisions/ADR-001-example.md",
        "# ADR-001: Example decision\n\nStatus: accepted\n\nBody.\n",
    );
    write("src/lib.rs", "pub fn f(x: i32) -> i32 {\n    x + 1\n}\n");
    write(
        ".craftsman/docs/manifest.json",
        "{\n  \"libraries\": {\n    \"demo\": {\n      \"source\": \"llms-txt\",\n      \"urls\": [\"https://example.dev/llms.txt\"],\n      \"version\": \"1.0.0\"\n    }\n  }\n}\n",
    );
    write(
        ".craftsman/docs/demo@1.0.0/pages/intro.md",
        "# Intro\n\nStreaming responses are the core feature.\n",
    );
    for git_args in [
        &["init", "--quiet"][..],
        &["add", "-A"][..],
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "init",
        ][..],
    ] {
        let status = Command::new("git")
            .args(git_args)
            .current_dir(dir)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {git_args:?} failed");
    }
    tmp
}

/// Assert `args` exits within `allowed` and stdout parses as JSON.
pub fn assert_json(dir: &Path, args: &[&str], home: Option<&Path>, allowed: &[i32]) {
    let out = craftsman(dir, args, home);
    let code = out.status.code().expect("exit code");
    assert!(
        allowed.contains(&code),
        "{args:?} exited {code}, allowed {allowed:?}:\n{}",
        combined(&out)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(
        parsed.is_ok(),
        "{args:?} stdout is not valid JSON:\n{stdout}"
    );
}

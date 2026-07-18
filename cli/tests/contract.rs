//! The command-surface contract sweep (Batch 8 audit, completed in Batch
//! 9b): every subcommand must have `--help` (exit-code documentation
//! where a verdict exists), answer a bad flag with exit 2, refuse a
//! missing config with exit 3, and emit parseable JSON on its happy path.
//!
//! JSON happy-path coverage by venue — every command in the surface is
//! now swept:
//! - here, offline: spec/plan/gate/docs add|status|search|get, arch,
//!   health, extract, adr, adopt, init, setup, update;
//! - `contract_offline.rs`: `security` + `mutate` (against pre-resolved
//!   hermetic tools — a loud skip when `~/.craftsman/tools` lacks them,
//!   so a fresh machine never downloads inside the sweep) and
//!   `docs sync` via a local `file` source (no network at all);
//! - elsewhere: `verify` + `commit` + `check-all` + `doctor` (compile
//!   the cucumber fixture — tests/spec.rs, tests/doctor.rs), `lint`
//!   (real cargo fmt/clippy — the SPEC gate scenarios), `mutate`
//!   score paths (tests/mutate.rs, live py/ts), `perf`/`a11y`/`visual`
//!   (`tests/runtime_gates.rs`, live red+green; their unconfigured exit-3
//!   contract is asserted here).

mod util;

use std::process::Command;

use util::{assert_json, combined, craftsman, fixture_project};

/// The full surface: every subcommand as an argv prefix, flagged `true`
/// when its exit code is a verdict contract (then `--help` must document
/// exit codes), `false` for report-only commands.
const SURFACE: &[(&[&str], bool)] = &[
    (&["init"], true),
    (&["import"], true),
    (&["adopt"], true),
    (&["setup"], true),
    (&["update"], true),
    (&["spec", "status"], false),
    (&["spec", "lint"], true),
    (&["spec", "gen"], true),
    (&["plan", "lint"], true),
    (&["verify"], true),
    (&["commit"], true),
    (&["lint"], true),
    (&["security"], true),
    (&["arch"], true),
    (&["health"], true),
    (&["mutate"], true),
    (&["perf"], true),
    (&["a11y"], true),
    (&["visual"], true),
    (&["check-all"], true),
    (&["gate", "status"], false),
    (&["gate", "baseline"], true),
    (&["gate", "strict"], true),
    (&["doctor"], true),
    (&["docs", "add"], false),
    (&["docs", "sync"], true),
    (&["docs", "status"], false),
    (&["docs", "search"], false),
    (&["docs", "get"], false),
    (&["extract"], false),
    (&["adr", "index"], false),
    (&["adr", "stale"], false),
];

#[test]
fn every_command_has_help() {
    let tmp = tempfile::tempdir().expect("tempdir");
    for (args, _) in SURFACE {
        let argv: Vec<&str> = args.iter().copied().chain(["--help"]).collect();
        let out = craftsman(tmp.path(), &argv, None);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{args:?} --help failed:\n{}",
            combined(&out)
        );
        assert!(
            !out.stdout.is_empty(),
            "{args:?} --help printed nothing to stdout"
        );
    }
}

#[test]
fn verdict_commands_document_exit_codes_in_help() {
    let tmp = tempfile::tempdir().expect("tempdir");
    for (args, _) in SURFACE.iter().filter(|(_, verdict)| *verdict) {
        let argv: Vec<&str> = args.iter().copied().chain(["--help"]).collect();
        let out = craftsman(tmp.path(), &argv, None);
        let help = String::from_utf8_lossy(&out.stdout).to_lowercase();
        assert!(
            help.contains("exit"),
            "{args:?} --help never mentions exit codes — agent-grade help \
             documents its verdict contract"
        );
    }
}

#[test]
fn a_bad_flag_is_a_usage_error_exit_2() {
    let tmp = tempfile::tempdir().expect("tempdir");
    for (args, _) in SURFACE {
        let argv: Vec<&str> = args.iter().copied().chain(["--no-such-flag"]).collect();
        let out = craftsman(tmp.path(), &argv, None);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{args:?} --no-such-flag must exit 2 (usage):\n{}",
            combined(&out)
        );
    }
}

/// Config-requiring commands in a dir with no craftsman.toml (and no
/// repo): orchestrator error, exit 3, naming the missing file.
#[test]
fn missing_config_is_an_orchestrator_error_exit_3() {
    let cases: &[&[&str]] = &[
        &["spec", "status"],
        &["spec", "lint"],
        &["spec", "gen"],
        &["plan", "lint"],
        &["verify"],
        &["lint"],
        &["security"],
        &["arch"],
        &["health"],
        &["mutate"],
        &["perf"],
        &["a11y"],
        &["visual"],
        &["check-all"],
        &["gate", "status"],
        &["gate", "baseline", "lint"],
        &["gate", "strict", "lint"],
        &["docs", "status"],
        &["docs", "search", "query"],
        &["docs", "get", "lib/page"],
        &["extract", "--show"],
        &["adr", "index"],
        &["adr", "stale"],
        &["commit", "--type", "chore", "--message", "x"],
    ];
    for args in cases {
        // An isolated dir per case: no ancestor may hold a craftsman.toml.
        let tmp = tempfile::tempdir().expect("tempdir");
        let out = craftsman(tmp.path(), args, None);
        assert_eq!(
            out.status.code(),
            Some(3),
            "{args:?} without a config must exit 3:\n{}",
            combined(&out)
        );
        assert!(
            combined(&out).contains("craftsman.toml"),
            "{args:?} error must name craftsman.toml:\n{}",
            combined(&out)
        );
    }
}

/// init and adopt do not need a config, but do need a repo: exit 3 with
/// a `git init` suggestion.
#[test]
fn commands_needing_a_repo_suggest_git_init() {
    for args in [
        &["init", "--name", "x", "--stack", "rust"][..],
        &["adopt", "--status"][..],
    ] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let out = craftsman(tmp.path(), args, None);
        assert_eq!(
            out.status.code(),
            Some(3),
            "{args:?} outside a repo must exit 3:\n{}",
            combined(&out)
        );
        assert!(
            combined(&out).contains("git init"),
            "{args:?} must suggest git init:\n{}",
            combined(&out)
        );
    }
}

/// Runtime gates refuse (exit 3) when their config section is absent —
/// the honest alternative to a silent green.
#[test]
fn unconfigured_runtime_gates_refuse_with_exit_3() {
    let tmp = fixture_project();
    for gate in ["perf", "a11y", "visual"] {
        let out = craftsman(tmp.path(), &[gate], None);
        assert_eq!(
            out.status.code(),
            Some(3),
            "{gate} without config must exit 3:\n{}",
            combined(&out)
        );
    }
}

#[test]
fn json_happy_paths_emit_parseable_json() {
    let tmp = fixture_project();
    let dir = tmp.path();
    let cases: &[&[&str]] = &[
        &["spec", "status", "--json"],
        &["spec", "lint", "--json"],
        &["plan", "lint", "--json"],
        &["gate", "status", "--json"],
        &["arch", "--json"],
        &["extract", "--json"],
        &["adr", "index", "--json"],
        &["adr", "stale", "--json"],
        &["docs", "status", "--json"],
        &["docs", "search", "streaming", "--json"],
        &["docs", "get", "demo/intro", "--json"],
        &[
            "docs",
            "add",
            "extra",
            "--source",
            "llms-txt",
            "--url",
            "https://example.dev/llms.txt",
            "--json",
        ],
        &["gate", "baseline", "health", "--json"],
        &["gate", "strict", "arch", "--json"],
    ];
    for args in cases {
        assert_json(dir, args, None, &[0]);
    }
    // Health may report findings (exit 1) — the JSON contract holds
    // either way.
    assert_json(dir, &["health", "--json"], None, &[0, 1]);
    // Adopt in the fixture repo.
    assert_json(dir, &["adopt", "--status", "--json"], None, &[0]);
    assert_json(dir, &["adopt", "--start-phase", "0", "--json"], None, &[0]);
}

#[test]
fn json_happy_paths_for_init_setup_update() {
    // init: its own fresh repo.
    let repo = tempfile::tempdir().expect("tempdir");
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(repo.path())
        .status()
        .expect("git init");
    assert!(status.success());
    assert_json(
        repo.path(),
        &["init", "--name", "demo", "--stack", "rust", "--json"],
        None,
        &[0],
    );

    // spec gen: bash stack generates without external tools.
    std::fs::write(
        repo.path().join("craftsman.toml"),
        "[project]\nname = \"demo\"\nstacks = [\"bash\"]\n\n[gates]\nverify = \"strict\"\n",
    )
    .expect("switch to bash stack");
    assert_json(repo.path(), &["spec", "gen", "--json"], None, &[0]);

    // setup + update: sandboxed HOME.
    let home = tempfile::tempdir().expect("home");
    std::fs::create_dir_all(home.path().join(".claude")).expect("marker");
    let home = Some(home.path());
    assert_json(repo.path(), &["setup", "--json"], home, &[0]);
    assert_json(repo.path(), &["setup", "--status", "--json"], home, &[0]);
    assert_json(repo.path(), &["update", "--json"], home, &[0]);
    assert_json(repo.path(), &["setup", "--remove", "--json"], home, &[0]);
}

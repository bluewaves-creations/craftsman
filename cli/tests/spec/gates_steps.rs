//! Step definitions — the recovered gate-mode, health, and doctor
//! scenarios (Batch 11): baseline blocking/ratchet semantics, the strict
//! flip, gate status, allow directives, duplication, and the round trip.

use std::process::Command;

use cucumber::{given, then, when};

use crate::CliWorld;

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("spawn git");
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

/// A function body longer than the fixture limit (max-function-lines 5).
fn long_fn(name: &str) -> String {
    format!(
        "pub fn {name}() {{\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    let d = 4;\n    let e = 5;\n    let _ = a + b + c + d + e;\n}}\n"
    )
}

const HEALTH_BASELINE_CONFIG: &str = "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\nhealth = \"baseline\"\n\n[health]\nmax-function-lines = 5\n";

#[given("a gate in baseline mode with 2 recorded findings")]
fn gate_with_two_baselined_findings(w: &mut CliWorld) {
    w.write("craftsman.toml", HEALTH_BASELINE_CONFIG);
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("src")).expect("mkdirs");
    w.write("src/alpha.rs", &long_fn("alpha"));
    w.write("src/beta.rs", &long_fn("beta"));
    w.write(".gitignore", ".craftsman/\n");
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["add", "-A"]);
    w.run_craftsman(&["gate", "baseline", "health"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "recording the baseline must pass:\n{}",
        w.combined_output()
    );
    assert_eq!(baseline_entries(w), 2, "the baseline must hold 2 entries");
}

/// Distinct fingerprints in `.craftsman/baselines/health.json`.
fn baseline_entries(w: &mut CliWorld) -> usize {
    let path = w.project_dir().join(".craftsman/baselines/health.json");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc: serde_json::Value = serde_json::from_str(&text).expect("baseline JSON");
    doc["fingerprints"].as_array().map_or(0, Vec::len)
}

#[given("the code now produces 1 of the recorded findings plus 1 fresh finding")]
fn code_with_one_old_one_fresh(w: &mut CliWorld) {
    w.write("src/alpha.rs", "pub fn alpha() {}\n");
    w.write("src/fresh.rs", &long_fn("fresh"));
    git(&w.project_dir(), &["add", "-A"]);
}

#[given("the code now produces only 1 of them")]
fn code_with_only_one_finding(w: &mut CliWorld) {
    w.write("src/alpha.rs", "pub fn alpha() {}\n");
    git(&w.project_dir(), &["add", "-A"]);
}

#[given("a changed-scope run produces no findings")]
fn changed_scope_clean(w: &mut CliWorld) {
    let dir = w.project_dir();
    git(
        &dir,
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "inherited debt",
        ],
    );
    w.write("src/innocent.rs", "pub fn innocent() {}\n");
    git(&dir, &["add", "-A"]);
}

#[when("the gate runs")]
fn gate_runs(w: &mut CliWorld) {
    w.run_craftsman(&["health"]);
}

#[when("the gate runs in full")]
fn gate_runs_full(w: &mut CliWorld) {
    w.run_craftsman(&["health"]);
}

#[when("the gate runs with changed scope")]
fn gate_runs_changed(w: &mut CliWorld) {
    w.run_craftsman(&["health", "--changed"]);
}

#[then("only the fresh finding is reported as blocking")]
fn only_fresh_finding_blocks(w: &mut CliWorld) {
    let combined = w.combined_output();
    let blocking: Vec<&str> = combined.lines().filter(|l| l.contains("FAIL")).collect();
    assert_eq!(
        blocking.len(),
        1,
        "expected exactly one blocking finding:\n{combined}"
    );
    assert!(
        blocking[0].contains("fresh.rs"),
        "the blocking finding must be the fresh one:\n{combined}"
    );
}

#[then("the baseline is ratcheted down to 1 entry")]
fn baseline_ratcheted_to_one(w: &mut CliWorld) {
    assert_eq!(baseline_entries(w), 1, "output:\n{}", w.combined_output());
}

#[then("the ratchet is recorded with a timestamp")]
fn ratchet_has_timestamp(w: &mut CliWorld) {
    let path = w.project_dir().join(".craftsman/baselines/health.json");
    let text = std::fs::read_to_string(&path).expect("baseline JSON");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("baseline JSON");
    assert!(
        doc["last_ratchet"].is_string(),
        "last_ratchet must carry a timestamp:\n{text}"
    );
}

#[then("the baseline still holds 2 entries")]
fn baseline_still_two(w: &mut CliWorld) {
    assert_eq!(baseline_entries(w), 2, "output:\n{}", w.combined_output());
}

const ARCH_BASELINE_CONFIG: &str =
    "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\narch = \"baseline\"\n";

#[given("a craftsman project whose arch gate is in baseline mode with zero baseline debt")]
fn arch_baseline_zero_debt(w: &mut CliWorld) {
    w.write("craftsman.toml", ARCH_BASELINE_CONFIG);
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("src")).expect("mkdirs");
    w.write("src/lib.rs", "pub fn tidy() {}\n");
    w.write(".gitignore", ".craftsman/\n");
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["add", "-A"]);
}

#[then("the config line for the arch gate now reads strict")]
fn arch_line_is_strict(w: &mut CliWorld) {
    let text =
        std::fs::read_to_string(w.project_dir().join("craftsman.toml")).expect("read config");
    assert!(
        text.contains("arch = \"strict\""),
        "the arch line was not flipped:\n{text}"
    );
}

#[then("no other config line changed")]
fn no_other_config_line_changed(w: &mut CliWorld) {
    let text =
        std::fs::read_to_string(w.project_dir().join("craftsman.toml")).expect("read config");
    let expected = ARCH_BASELINE_CONFIG.replace("arch = \"baseline\"", "arch = \"strict\"");
    assert_eq!(text, expected, "the flip touched more than the gate line");
}

#[given("a craftsman project whose config sets verify to strict and lint to baseline")]
fn config_with_two_modes(w: &mut CliWorld) {
    w.write(
        "craftsman.toml",
        "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\nverify = \"strict\"\nlint = \"baseline\"\n",
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
}

#[then("the output lists 9 gates each with a mode and a baseline count")]
fn output_lists_nine_gates(w: &mut CliWorld) {
    let combined = w.combined_output();
    let rows = combined
        .lines()
        .filter(|l| {
            let mut parts = l.split_whitespace();
            parts.next().is_some()
                && parts.next().is_some()
                && parts.next().is_some_and(|n| n.parse::<u64>().is_ok())
        })
        .count();
    assert_eq!(rows, 9, "expected 9 gate rows:\n{combined}");
}

const HEALTH_STRICT_CONFIG: &str = "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\nhealth = \"strict\"\n\n[health]\nmax-function-lines = 5\n";

fn health_project_with(w: &mut CliWorld, source: &str) {
    w.write("craftsman.toml", HEALTH_STRICT_CONFIG);
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("src")).expect("mkdirs");
    w.write("src/lib.rs", source);
    w.write(".gitignore", ".craftsman/\n");
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["add", "-A"]);
}

#[given(
    "a source file whose over-long function is preceded by a craftsman-health allow directive carrying a reason"
)]
fn allow_with_reason(w: &mut CliWorld) {
    let source = format!(
        "// craftsman-health: allow max-function-lines — inherited fixture, shrinks in batch 2\n{}",
        long_fn("sprawling")
    );
    health_project_with(w, &source);
}

#[given(
    "a source file whose over-long function is preceded by a craftsman-health allow directive with no reason"
)]
fn allow_without_reason(w: &mut CliWorld) {
    let source = format!(
        "// craftsman-health: allow max-function-lines\n{}",
        long_fn("sprawling")
    );
    health_project_with(w, &source);
}

#[then("no finding is reported for that function")]
fn no_finding_for_function(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        !combined.contains("FAIL"),
        "the allowed function was still reported:\n{combined}"
    );
}

#[then("a finding reports the reasonless allow directive")]
fn finding_for_reasonless_allow(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("allow-directive"),
        "no allow-directive finding in:\n{combined}"
    );
}

#[then("the over-long function is still reported")]
fn long_function_still_reported(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("max-function-lines"),
        "the long function must stay a finding:\n{combined}"
    );
}

#[given("two source files sharing an identical 12-line block")]
fn two_files_with_shared_block(w: &mut CliWorld) {
    let block = "pub fn shared() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    let d = 4;\n    let e = 5;\n    let f = 6;\n    let g = 7;\n    let h = 8;\n    let i = 9;\n    let j = 10;\n    let _ = a + b + c + d + e + f + g + h + i + j;\n}\n";
    w.write(
        "craftsman.toml",
        "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\nhealth = \"strict\"\n",
    );
    w.write(
        "SPEC.md",
        "Feature: Fixture feature\n\n  Scenario: First behavior\n",
    );
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("src")).expect("mkdirs");
    w.write("src/dup_a.rs", block);
    w.write("src/dup_b.rs", &format!("// twin copy\n{block}"));
    w.write(".gitignore", ".craftsman/\n");
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["add", "-A"]);
}

#[then("one duplication finding names both locations")]
fn duplication_names_both(w: &mut CliWorld) {
    let combined = w.combined_output();
    let both = combined
        .lines()
        .filter(|l| l.contains("duplication") && l.contains("dup_a.rs") && l.contains("dup_b.rs"))
        .count();
    assert!(
        both >= 1,
        "no duplication finding names both files:\n{combined}"
    );
}

#[given("a craftsman project whose tools are installed")]
fn project_with_tools_installed(w: &mut CliWorld) {
    let dir = w.project_dir();
    git(&dir, &["init", "--quiet"]);
    let tools = dir.join(".craftsman/fixture-tools");
    for pin in ["gitleaks@8.24.0", "osv-scanner@2.4.0"] {
        std::fs::create_dir_all(tools.join(pin)).expect("mkdirs");
    }
    w.env.push((
        "CRAFTSMAN_TOOLS_DIR".to_owned(),
        tools.display().to_string(),
    ));
    w.run_craftsman(&["init", "--name", "demo", "--stack", "rust"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "priming init must pass:\n{}",
        w.combined_output()
    );
}

#[then("the round-trip check reports both a red and a green observation")]
fn round_trip_red_then_green(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("red observed") && combined.contains("green observed"),
        "the round-trip detail must report both phases:\n{combined}"
    );
}

//! Step definitions — the security scenarios (Batch 11 + the Batch 12
//! delta promises): the secret-hiding scan, the threshold partition, and
//! the broken-scanner refusal. Hermetic like `tests/security_gates.rs`:
//! the scanners are fakes in a sandboxed tools dir, semgrep behind a fake
//! `uvx` PATH shim.

use std::os::unix::fs::PermissionsExt as _;

use cucumber::given;

use crate::CliWorld;

/// The committed secret the output must never reveal — assembled at run
/// time so this repository's own history never contains an AWS-key-shaped
/// literal for the real gitleaks scan to flag.
fn planted_secret() -> String {
    format!("{}{}", "AKIA", "FIXTUREFAKEKEY99")
}

/// A fake gitleaks that writes `report` where asked and exits `code`.
fn gitleaks_script(report: &str, code: u8) -> String {
    format!(
        "#!/bin/sh\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--report-path\" ]; then shift; printf '%s' '{report}' > \"$1\"; fi\n  shift\ndone\nexit {code}\n"
    )
}

/// Semgrep JSON a fake `uvx` emits: one ERROR finding (mapped to high).
const SEMGREP_ONE_HIGH: &str = r#"{"results":[{"check_id":"fixture.rule","path":"src/main.rs","start":{"line":3},"extra":{"severity":"ERROR","message":"seeded high finding"}}],"errors":[]}"#;

const SEMGREP_CLEAN: &str = r#"{"results":[],"errors":[]}"#;

fn exec_script(path: &std::path::Path, content: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdirs");
    std::fs::write(path, content).expect("write script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// Plant the fake scanner sandbox: gitleaks + osv-scanner + the pinned
/// ruleset in a `CRAFTSMAN_TOOLS_DIR`, and semgrep behind a `uvx` shim.
fn install_fake_scanners(w: &mut CliWorld, gitleaks: &str, semgrep_json: &str) {
    let dir = w.project_dir();
    let tools = dir.join(".craftsman/fixture-tools");
    exec_script(&tools.join("gitleaks@8.24.0/gitleaks"), gitleaks);
    exec_script(
        &tools.join("osv-scanner@2.4.0/osv-scanner"),
        "#!/bin/sh\nexit 0\n",
    );
    std::fs::create_dir_all(tools.join("semgrep-rules@1.146.0")).expect("mkdirs");
    std::fs::write(
        tools.join("semgrep-rules@1.146.0/default.yaml"),
        "rules: []\n",
    )
    .expect("ruleset");
    let shim = dir.join(".craftsman/fixture-shim");
    exec_script(
        &shim.join("uvx"),
        &format!("#!/bin/sh\nprintf '%s' '{semgrep_json}'\nexit 0\n"),
    );
    w.env.push((
        "CRAFTSMAN_TOOLS_DIR".to_owned(),
        tools.display().to_string(),
    ));
    w.env.push((
        "PATH".to_owned(),
        format!(
            "{}:{}",
            shim.display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    ));
}

fn security_project(w: &mut CliWorld, threshold: Option<&str>) {
    let threshold_line = threshold
        .map(|t| format!("security-threshold = \"{t}\"\n"))
        .unwrap_or_default();
    w.write(
        "craftsman.toml",
        &format!(
            "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\nsecurity = \"strict\"\n{threshold_line}"
        ),
    );
    w.write(".gitignore", ".craftsman/\n");
}

#[given("a repository whose history contains a committed API key")]
fn repo_with_committed_key(w: &mut CliWorld) {
    security_project(w, None);
    let secret = planted_secret();
    std::fs::create_dir_all(w.project_dir().join("config")).expect("mkdirs");
    w.write("config/prod.env", &format!("AWS_ACCESS_KEY_ID={secret}\n"));
    crate::repo_steps::git_init_commit_all(&w.project_dir());
    // The gitleaks report a real scan of that history would produce —
    // secret value included, exactly what the parser must hide.
    let report = format!(
        r#"[{{"RuleID":"aws-access-key","File":"config/prod.env","StartLine":1,"Description":"AWS Access Key","Secret":"{secret}","Commit":"deadbeef"}}]"#
    );
    install_fake_scanners(w, &gitleaks_script(&report, 1), SEMGREP_CLEAN);
}

#[given("a craftsman project whose security scan reports one finding below the threshold")]
fn scan_with_below_threshold_finding(w: &mut CliWorld) {
    // Threshold critical; the fake semgrep HIGH finding sits below it.
    security_project(w, Some("critical"));
    crate::repo_steps::git_init_commit_all(&w.project_dir());
    install_fake_scanners(w, &gitleaks_script("[]", 0), SEMGREP_ONE_HIGH);
}

#[given("a craftsman project whose security scanner exits with an unexpected code")]
fn scanner_with_unexpected_exit(w: &mut CliWorld) {
    security_project(w, None);
    crate::repo_steps::git_init_commit_all(&w.project_dir());
    install_fake_scanners(w, "#!/bin/sh\necho boom >&2\nexit 2\n", SEMGREP_CLEAN);
}

#[cucumber::then("the output names the broken scanner")]
fn output_names_broken_scanner(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("gitleaks"),
        "the refusal must name the broken scanner:\n{combined}"
    );
}

#[cucumber::then("the finding names the file and rule")]
fn finding_names_file_and_rule(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("prod.env") && combined.contains("aws-access-key"),
        "the finding must name the file and rule:\n{combined}"
    );
}

#[cucumber::then("the output does not contain the secret value")]
fn output_hides_secret(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        !combined.contains(&planted_secret()),
        "the secret leaked into the output:\n{combined}"
    );
}

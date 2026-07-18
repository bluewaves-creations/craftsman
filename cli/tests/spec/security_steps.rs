//! Step definitions — the recovered security scenario (Batch 11): a
//! committed API key is detected and reported without ever printing the
//! secret. Hermetic like `tests/security_gates.rs`: the scanners are
//! fakes in a sandboxed tools dir, semgrep behind a fake `uvx` PATH shim.

use std::os::unix::fs::PermissionsExt as _;

use cucumber::given;

use crate::CliWorld;

/// The committed secret the output must never reveal — assembled at run
/// time so this repository's own history never contains an AWS-key-shaped
/// literal for the real gitleaks scan to flag.
fn planted_secret() -> String {
    format!("{}{}", "AKIA", "FIXTUREFAKEKEY99")
}

fn exec_script(path: &std::path::Path, content: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdirs");
    std::fs::write(path, content).expect("write script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

#[given("a repository whose history contains a committed API key")]
fn repo_with_committed_key(w: &mut CliWorld) {
    w.write(
        "craftsman.toml",
        "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\nsecurity = \"strict\"\n",
    );
    w.write(".gitignore", ".craftsman/\n");
    let secret = planted_secret();
    std::fs::create_dir_all(w.project_dir().join("config")).expect("mkdirs");
    w.write("config/prod.env", &format!("AWS_ACCESS_KEY_ID={secret}\n"));
    let dir = w.project_dir();
    crate::repo_steps::git_init_commit_all(&dir);

    // The gitleaks report a real scan of that history would produce —
    // secret value included, exactly what the parser must hide.
    let report = format!(
        r#"[{{"RuleID":"aws-access-key","File":"config/prod.env","StartLine":1,"Description":"AWS Access Key","Secret":"{secret}","Commit":"deadbeef"}}]"#
    );
    let tools = dir.join(".craftsman/fixture-tools");
    exec_script(
        &tools.join("gitleaks@8.24.0/gitleaks"),
        &format!(
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--report-path\" ]; then shift; printf '%s' '{report}' > \"$1\"; fi\n  shift\ndone\nexit 1\n"
        ),
    );
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
        "#!/bin/sh\nprintf '%s' '{\"results\":[],\"errors\":[]}'\nexit 0\n",
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

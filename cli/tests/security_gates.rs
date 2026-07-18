//! GAP-R05 / GAP-R06 characterization pins for the security gate, fully
//! hermetic: the scanners are fakes — gitleaks/osv-scanner planted in a
//! sandboxed `CRAFTSMAN_TOOLS_DIR` (the hermetic resolve path picks any
//! existing binary), semgrep behind a fake `uvx` on a PATH shim, and the
//! pinned ruleset pre-planted so nothing downloads. The fixture repo has
//! no lockfiles, so osv-scanner resolves but never runs.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;

fn write_exec(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdirs");
    std::fs::write(path, content).expect("write script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// Semgrep JSON a fake `uvx` emits: one ERROR finding (mapped to high).
const SEMGREP_ONE_HIGH: &str = r#"{"results":[{"check_id":"fixture.rule","path":"src/main.rs","start":{"line":3},"extra":{"severity":"ERROR","message":"seeded high finding"}}],"errors":[]}"#;

struct Sandbox {
    project: tempfile::TempDir,
    tools: tempfile::TempDir,
    shim: tempfile::TempDir,
}

/// A git project with strict security, fake hermetic scanners, and a fake
/// `uvx`; `gitleaks_script` controls the interesting scanner's behavior.
fn sandbox(threshold: Option<&str>, gitleaks_script: &str) -> Sandbox {
    let project = tempfile::tempdir().expect("project");
    let tools = tempfile::tempdir().expect("tools");
    let shim = tempfile::tempdir().expect("shim");

    let threshold_line = threshold
        .map(|t| format!("security-threshold = \"{t}\"\n"))
        .unwrap_or_default();
    std::fs::write(
        project.path().join("craftsman.toml"),
        format!(
            "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n\n[gates]\nsecurity = \"strict\"\n{threshold_line}"
        ),
    )
    .expect("config");
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(project.path())
            .status()
            .expect("git")
            .success()
    );

    write_exec(
        &tools.path().join("gitleaks@8.24.0/gitleaks"),
        gitleaks_script,
    );
    write_exec(
        &tools.path().join("osv-scanner@2.4.0/osv-scanner"),
        "#!/bin/sh\nexit 0\n",
    );
    std::fs::create_dir_all(tools.path().join("semgrep-rules@1.146.0")).expect("mkdirs");
    std::fs::write(
        tools.path().join("semgrep-rules@1.146.0/default.yaml"),
        "rules: []\n",
    )
    .expect("ruleset");

    write_exec(
        &shim.path().join("uvx"),
        &format!("#!/bin/sh\nprintf '%s' '{SEMGREP_ONE_HIGH}'\nexit 0\n"),
    );

    Sandbox {
        project,
        tools,
        shim,
    }
}

fn run_security(sb: &Sandbox) -> std::process::Output {
    let path = format!(
        "{}:{}",
        sb.shim.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new(env!("CARGO_BIN_EXE_craftsman"))
        .args(["security", "--json"])
        .current_dir(sb.project.path())
        .env("CRAFTSMAN_TOOLS_DIR", sb.tools.path())
        .env("PATH", path)
        .output()
        .expect("spawn craftsman security")
}

/// GAP-R05 pin: findings below the configured threshold inform — they
/// appear in the report — but never block, and the gate says so.
#[test]
fn below_threshold_findings_inform_but_never_block() {
    // Fake gitleaks: clean report. The fake semgrep HIGH finding sits
    // below the critical threshold.
    let sb = sandbox(
        Some("critical"),
        "#!/bin/sh\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--report-path\" ]; then shift; printf '[]' > \"$1\"; fi\n  shift\ndone\nexit 0\n",
    );
    let output = run_security(&sb);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "informational findings must never fail the gate:\n{stderr}"
    );
    assert!(
        stderr.contains("below the critical threshold (informational)"),
        "the partition must be announced:\n{stderr}"
    );
    let doc: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("gate JSON");
    assert_eq!(doc["passed"], true, "{doc:#}");
    assert_eq!(doc["blocking"], 0, "{doc:#}");
    assert_eq!(
        doc["findings"].as_array().map(Vec::len),
        Some(1),
        "informational findings stay visible: {doc:#}"
    );
}

/// GAP-R06 pin: a scanner that breaks (unexpected exit code) is exit 3 —
/// an orchestrator error naming the tool — never a green gate.
#[test]
fn broken_scanner_is_exit_3_never_green() {
    let sb = sandbox(None, "#!/bin/sh\necho boom >&2\nexit 2\n");
    let output = run_security(&sb);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(3),
        "a broken scanner is an orchestrator error:\n{stderr}"
    );
    assert!(
        stderr.contains("gitleaks") && stderr.contains('2'),
        "the failure must name the tool and exit code:\n{stderr}"
    );
}

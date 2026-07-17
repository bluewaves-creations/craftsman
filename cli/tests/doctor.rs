//! Doctor round-trip e2e: drives the compiled binary against this repo and
//! asserts every check — including the red→green fixture round trip — is
//! green. This also closes Batch 2's honest-undone gap: a real failing
//! scenario observed as exit-1-grade `Failed` end-to-end. The fixture is
//! cached under the system temp dir; observed timings on this machine:
//! ~15s cold, ~2s cached — under the ~90s budget, so it runs unconditionally
//! (the plan's fallback of a `CRAFTSMAN_E2E` gate was not needed).

use std::path::Path;
use std::process::Command;

#[test]
fn doctor_round_trip_observes_red_then_green() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli/ lives one level under the repo root");
    let output = Command::new(env!("CARGO_BIN_EXE_craftsman"))
        .args(["doctor", "--json"])
        .current_dir(repo_root)
        .output()
        .expect("spawn craftsman doctor");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "doctor must exit 0 on this repo\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Human report first, then the --json document: parse the JSON tail.
    let json_start = stdout.find("{\n").expect("a JSON document on stdout");
    let doc: serde_json::Value =
        serde_json::from_str(&stdout[json_start..]).expect("valid doctor JSON");
    assert_eq!(doc["passed"], true, "{doc:#}");

    let checks = doc["checks"].as_array().expect("a checks array");
    let round_trip = checks
        .iter()
        .find(|c| c["name"] == "round-trip")
        .expect("a round-trip check");
    assert_eq!(round_trip["passed"], true, "{round_trip:#}");
    let detail = round_trip["detail"].as_str().expect("detail string");
    assert!(
        detail.contains("red observed") && detail.contains("green observed"),
        "round trip must observe red then green: {detail}"
    );
}

//! Live round trip for the xcodebuild variant of the swift stack
//! (Batch 9a), driven against the committed fixture package
//! `tests/fixtures/xcode-app` (whose craftsman.toml sets
//! `[verify.swift] scheme` — see the fixture for the probed
//! package-scheme facts).
//!
//! Measured on this machine (Xcode 26.6, M-series): ~42s cold, ~9s warm
//! per phase — cold `DerivedData` busts the 90s budget, so the round trip
//! is `#[ignore]`-gated per the plan; the unignored fast path is the
//! parser test over `fixtures/xcresult-tests.json`
//! (`tests/normalize_fixtures.rs`).

use std::path::Path;
use std::process::Command;

use craftsman::verify::normalize::Status;
use craftsman::verify::{self, Outcome, Selection};

#[test]
#[ignore = "drives xcodebuild (~40s cold) — run with `cargo test -- --ignored` on a Mac with Xcode 16+"]
fn xcodebuild_round_trip_pass_undefined_fail() {
    // xcodebuild spells it `-version` (single dash), unlike every other tool.
    let xcodebuild_runnable = Command::new("xcodebuild")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success());
    assert!(
        xcodebuild_runnable,
        "ignored test explicitly requested but xcodebuild is not runnable"
    );
    // Stable path: DerivedData is keyed by project path, so reruns stay warm.
    let dir = std::env::temp_dir().join("craftsman-xcodebuild-roundtrip-fixture");
    let fixture = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/xcode-app"
    ));
    copy_tree(fixture, &dir);
    let _ = std::fs::remove_dir_all(dir.join(".craftsman"));

    let green = verify::run(&dir, &Selection::All).expect("verify runs");
    let mut got = statuses(&green);
    got.sort();
    assert_eq!(
        got,
        vec![
            (
                "Quantities within range are accepted".to_owned(),
                Status::Passed
            ),
            ("Scenario A passes".to_owned(), Status::Passed),
            ("Scenario B stays undefined".to_owned(), Status::Undefined),
        ],
        "green phase through the xcresult bundle"
    );
    assert_eq!(
        green.outcome,
        Outcome::Failed,
        "undefined keeps the gate red"
    );

    // Break A and select it alone — proves the -only-testing recipe.
    let steps = dir.join("Tests/XcodeAppTests/Steps.swift");
    let text = std::fs::read_to_string(&steps).expect("read steps");
    std::fs::write(&steps, text.replace("counter == 2", "counter == 3")).expect("write steps");
    let red = verify::run(&dir, &Selection::Scenario("Scenario A passes".to_owned()))
        .expect("verify runs");
    assert_eq!(
        statuses(&red),
        vec![("Scenario A passes".to_owned(), Status::Failed)],
        "red phase selected one test via -only-testing"
    );
}

fn statuses(report: &verify::Report) -> Vec<(String, Status)> {
    report
        .results()
        .map(|r| (r.scenario.clone(), r.status))
        .collect()
}

/// Minimal recursive copy for fixture trees. Content-stable writes keep
/// mtimes (and so build caches) warm across reruns.
fn copy_tree(from: &Path, to: &Path) {
    let entries = std::fs::read_dir(from).unwrap_or_else(|e| panic!("{}: {e}", from.display()));
    for entry in entries.flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree(&src, &dst);
            continue;
        }
        let content =
            std::fs::read_to_string(&src).unwrap_or_else(|e| panic!("{}: {e}", src.display()));
        if std::fs::read_to_string(&dst).is_ok_and(|existing| existing == content) {
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("mkdir {}: {e}", parent.display()));
        }
        std::fs::write(&dst, content).unwrap_or_else(|e| panic!("write {}: {e}", dst.display()));
    }
}

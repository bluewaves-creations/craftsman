//! Step definitions — the recovered runtime-gate scenarios (Batch 11,
//! `@requires-chromium`): the visual drift and seeded-a11y-issue red
//! verdicts on the static-site fixture, through real Playwright runs.

use std::process::Command;

use cucumber::{given, then};

use crate::{CliWorld, fixtures};

/// The static-site fixture at a stable path with dependencies installed
/// (`bun install --frozen-lockfile` once; `node_modules` survives runs).
fn site_fixture(w: &mut CliWorld, dir_name: &str, gate_section: &str) {
    let dir = fixtures::stable_dir(dir_name);
    fixtures::copy_tree(&crate::probes::static_site_fixture(), &dir);
    fixtures::scrub(&dir, &[".craftsman"]);
    std::fs::write(
        dir.join("craftsman.toml"),
        format!("[project]\nname = \"static-site\"\nstacks = [\"typescript\"]\n\n{gate_section}\n"),
    )
    .expect("write craftsman.toml");
    if !dir.join("node_modules").is_dir() {
        let status = Command::new("bun")
            .args(["install", "--frozen-lockfile"])
            .current_dir(&dir)
            .status()
            .expect("spawn bun install");
        assert!(status.success(), "bun install failed in {}", dir.display());
    }
    w.fixed_dir = Some(dir);
}

#[given(
    "a craftsman project with a configured visual gate whose page drifted from its committed baseline"
)]
fn visual_gate_with_drift(w: &mut CliWorld) {
    site_fixture(
        w,
        "craftsman-spec-visual-fixture",
        "[visual]\ntest-glob = \"tests/visual-broken.spec.ts\"",
    );
}

#[then("a failed-spec finding names the failing spec file")]
fn failed_spec_names_file(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("failed-spec") && combined.contains("visual-broken.spec.ts"),
        "the finding must name the failing spec:\n{combined}"
    );
}

#[given(
    "a craftsman project with a configured a11y gate whose page carries a seeded accessibility issue"
)]
fn a11y_gate_with_seeded_issue(w: &mut CliWorld) {
    site_fixture(
        w,
        "craftsman-spec-a11y-fixture",
        "[a11y]\ntest-glob = \"tests/a11y-broken.spec.ts\"",
    );
}

#[then("a failed-spec finding is reported with its line")]
fn failed_spec_has_line(w: &mut CliWorld) {
    let combined = w.combined_output();
    let with_line = combined.lines().any(|l| {
        l.contains("failed-spec")
            && l.split("a11y-broken.spec.ts:")
                .nth(1)
                .is_some_and(|rest| rest.chars().next().is_some_and(|c| c.is_ascii_digit()))
    });
    assert!(
        with_line,
        "the finding must carry file:line evidence:\n{combined}"
    );
}

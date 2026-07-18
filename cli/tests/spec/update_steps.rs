//! Step definitions — the `craftsman update` scenarios (Batch 10):
//! receipt-driven self-update, hermetic paths, and the live
//! `@requires-network` flow.

use cucumber::{given, then};

use crate::CliWorld;

/// The receipt directory axoupdater consults under the sandboxed home
/// (homedir resolves `$HOME` first; XDG/AXOUPDATER overrides are removed
/// by `run_craftsman`).
fn receipt_dir(home: &std::path::Path) -> std::path::PathBuf {
    home.join(".config/craftsman")
}

#[given("a home directory with no craftsman install receipt")]
fn home_without_receipt(w: &mut CliWorld) {
    w.home = Some(tempfile::tempdir().expect("home tempdir"));
    let _ = w.project_dir();
}

#[given("a home directory with an outdated craftsman skill installed")]
fn home_with_outdated_skill(w: &mut CliWorld) {
    use craftsman::bootstrap::setup;

    let home = tempfile::tempdir().expect("home tempdir");
    setup::install(home.path(), false).expect("baseline skill install");
    // Age one skill the way an older release would have left it: stale
    // content, but a sentinel that still proves setup wrote the tree.
    let skill = home.path().join(".agents/skills/craftsman-spec");
    std::fs::write(
        skill.join("SKILL.md"),
        "# Craftsman Spec\n\nOutdated payload.\n",
    )
    .expect("age SKILL.md");
    let digest = setup::tree_digest(&skill).expect("digest aged tree");
    let sentinel = skill.join(".craftsman-setup");
    let first = std::fs::read_to_string(&sentinel)
        .expect("sentinel")
        .lines()
        .next()
        .expect("sentinel line 1")
        .to_owned();
    std::fs::write(&sentinel, format!("{first}\n{digest}\n")).expect("re-attest aged tree");
    w.home = Some(home);
    let _ = w.project_dir();
}

#[then("the installed skill matches the binary's embedded copy")]
fn installed_skill_matches_embedded(w: &mut CliWorld) {
    let home = w.home.as_ref().expect("sandboxed home").path();
    let installed = std::fs::read_to_string(home.join(".agents/skills/craftsman-spec/SKILL.md"))
        .expect("installed SKILL.md");
    let embedded = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../skills/craftsman-spec/SKILL.md"
    ))
    .expect("repo copy of SKILL.md");
    assert_eq!(installed, embedded, "update must refresh the aged skill");
}

#[given("a home directory with a craftsman install receipt for an unreachable release source")]
fn home_with_unreachable_receipt(w: &mut CliWorld) {
    let home = tempfile::tempdir().expect("home tempdir");
    let dir = receipt_dir(home.path());
    std::fs::create_dir_all(&dir).expect("receipt dir");
    // install_prefix must be the binary-under-test's own directory, or the
    // receipt-ownership check declines before ever touching the channel.
    let prefix = std::path::Path::new(env!("CARGO_BIN_EXE_craftsman"))
        .parent()
        .expect("binary dir")
        .to_str()
        .expect("utf-8 path")
        .to_owned();
    std::fs::write(
        dir.join("craftsman-receipt.json"),
        format!(
            r#"{{"install_prefix":"{prefix}","binaries":["craftsman"],"source":{{"release_type":"github","owner":"bluewaves-creations","name":"craftsman","app_name":"craftsman"}},"version":"0.0.1","provider":{{"source":"cargo-dist","version":"0.32.0"}}}}"#
        ),
    )
    .expect("write receipt");
    w.home = Some(home);
    // Dead endpoint: the channel probe fails deterministically, offline.
    w.env.push((
        "CRAFTSMAN_INSTALLER_GITHUB_BASE_URL".to_owned(),
        "http://127.0.0.1:9".to_owned(),
    ));
    let _ = w.project_dir();
}

#[then("the output names the current version")]
fn output_names_current_version(w: &mut CliWorld) {
    let combined = w.combined_output();
    let expected = concat!("craftsman ", env!("CARGO_PKG_VERSION"));
    assert!(
        combined.contains(expected),
        "output must name {expected:?}:\n{combined}"
    );
}

#[then("the output names the release channel")]
fn output_names_release_channel(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("github:bluewaves-creations/craftsman"),
        "output must name the receipt's channel:\n{combined}"
    );
}

#[then("the output does not claim success")]
fn output_does_not_claim_success(w: &mut CliWorld) {
    let combined = w.combined_output();
    for claim in ["is the latest release", "installed to"] {
        assert!(
            !combined.contains(claim),
            "output must not claim success ({claim:?}):\n{combined}"
        );
    }
}

/// LIVE ONLY (`@requires-network`, gated by `CRAFTSMAN_LIVE=1`): plants a
/// receipt claiming this binary came from an older release, so `update`
/// really downloads the latest GitHub Release and replaces the test
/// binary. Cargo rebuilds it on the next build; run this scenario alone
/// (`craftsman verify --scenario …`).
#[given("craftsman was installed from a GitHub release older than the latest")]
fn installed_from_older_release(w: &mut CliWorld) {
    let dir = tempfile::tempdir().expect("receipt tempdir");
    let prefix = std::path::Path::new(env!("CARGO_BIN_EXE_craftsman"))
        .parent()
        .expect("binary dir")
        .to_str()
        .expect("utf-8 path")
        .to_owned();
    std::fs::write(
        dir.path().join("craftsman-receipt.json"),
        format!(
            r#"{{"install_prefix":"{prefix}","binaries":["craftsman"],"source":{{"release_type":"github","owner":"bluewaves-creations","name":"craftsman","app_name":"craftsman"}},"version":"0.0.1","provider":{{"source":"cargo-dist","version":"0.32.0"}}}}"#
        ),
    )
    .expect("write receipt");
    w.env.push((
        "AXOUPDATER_CONFIG_PATH".to_owned(),
        dir.path().to_str().expect("utf-8 path").to_owned(),
    ));
    // Keep the tempdir alive for the scenario.
    w.dir.get_or_insert(dir);
    let _ = w.project_dir();
}

#[then("the reported version afterwards equals the latest release version")]
fn reported_version_equals_latest(w: &mut CliWorld) {
    // The machine verdict: a second run must find nothing left to update.
    w.run_craftsman(&["update", "--json"]);
    let stdout = String::from_utf8_lossy(&w.output().stdout);
    let doc: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("update --json stdout is not JSON ({e}):\n{stdout}"));
    assert_eq!(
        doc["self_update"]["status"], "up-to-date",
        "after a self-update the channel must report up-to-date:\n{stdout}"
    );
}

//! Bootstrap integration tests: adopt's state machine over a real repo,
//! and setup's attribution semantics in a sandboxed home (Batch 8).

use std::process::Command;

use craftsman::bootstrap::adopt::{self, ADOPTION_REL, AdoptError, Adoption};
use craftsman::bootstrap::setup;

fn repo() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(tmp.path())
        .status()
        .expect("git init");
    assert!(status.success());
    tmp
}

#[test]
fn adopt_status_before_any_phase_reports_not_started() {
    let tmp = repo();
    let report = adopt::status(tmp.path()).expect("status");
    assert!(report.phases.is_empty());
    assert_eq!(report.next_phase, Some(0));
}

#[test]
fn adopt_enforces_sequencing_and_records_transitions() {
    let tmp = repo();
    let err = adopt::start_phase(tmp.path(), 2).expect_err("2 before 0/1");
    assert!(
        matches!(
            err,
            AdoptError::OutOfOrder {
                phase: 2,
                blocker: 0
            }
        ),
        "{err}"
    );

    adopt::start_phase(tmp.path(), 0).expect("start 0");
    let err = adopt::start_phase(tmp.path(), 1).expect_err("1 before 0 completes");
    assert!(
        matches!(err, AdoptError::OutOfOrder { blocker: 0, .. }),
        "{err}"
    );
    let err = adopt::start_phase(tmp.path(), 0).expect_err("0 twice");
    assert!(matches!(err, AdoptError::AlreadyRecorded { .. }), "{err}");

    adopt::complete_phase(tmp.path(), 0).expect("complete 0");
    let report = adopt::start_phase(tmp.path(), 1).expect("start 1 now allowed");
    assert_eq!(report.next_phase, Some(1));
    let record = &report.phases[0];
    assert!(record.completed_at.is_some());
    assert_eq!(record.started_head, "none", "pre-first-commit HEAD");

    // Phase 1 wrote the mechanical scaffold.
    assert!(tmp.path().join("craftsman.toml").is_file());
    assert!(
        tmp.path()
            .join("decisions/ADR-000-adoption-baseline.md")
            .is_file()
    );
    let text = std::fs::read_to_string(tmp.path().join(ADOPTION_REL)).expect("state");
    let parsed: Adoption = toml::from_str(&text).expect("state parses");
    assert_eq!(parsed.phases.len(), 2);
}

#[test]
fn adopt_complete_requires_start() {
    let tmp = repo();
    let err = adopt::complete_phase(tmp.path(), 3).expect_err("never started");
    assert!(matches!(err, AdoptError::NotStarted { phase: 3 }), "{err}");
}

#[test]
fn adopt_phase_1_leaves_an_existing_config_alone() {
    let tmp = repo();
    std::fs::write(
        tmp.path().join("craftsman.toml"),
        "[project]\nname = \"keepme\"\nstacks = [\"rust\"]\n",
    )
    .expect("seed config");
    adopt::start_phase(tmp.path(), 0).expect("start 0");
    adopt::complete_phase(tmp.path(), 0).expect("complete 0");
    let report = adopt::start_phase(tmp.path(), 1).expect("start 1");
    assert!(report.actions.iter().any(|a| a.contains("left untouched")));
    let text = std::fs::read_to_string(tmp.path().join("craftsman.toml")).expect("read");
    assert!(text.contains("keepme"), "config must be untouched");
}

/// A sandboxed home with Claude Code (link mode) and Codex (standard
/// mode) markers.
fn sandbox_home() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(".claude")).expect("claude marker");
    std::fs::create_dir_all(tmp.path().join(".codex")).expect("codex marker");
    tmp
}

#[test]
fn setup_installs_canonical_copies_with_sentinels_and_links_agents() {
    let tmp = sandbox_home();
    let home = tmp.path();
    let report = setup::install(home, false).expect("install");
    let canonical = setup::canonical_dir(home);

    assert!(
        canonical.join("craftsman-init/SKILL.md").is_file(),
        "canonical skill extracted"
    );
    let sentinel = std::fs::read_to_string(canonical.join("craftsman-init/.craftsman-setup"))
        .expect("sentinel written");
    assert_eq!(
        sentinel.lines().nth(1).map(str::to_owned),
        Some(setup::tree_digest(&canonical.join("craftsman-init")).expect("digest")),
        "sentinel records the tree sha256"
    );
    assert!(
        home.join(".claude/skills/craftsman-init").is_symlink(),
        "Claude Code gets symlinks"
    );
    assert!(
        report
            .rows
            .iter()
            .any(|r| r.scope == "Codex" && r.action == "standard"),
        "standard-mode agents get an advisory row"
    );

    // A second run is a no-op: everything up-to-date.
    let report = setup::install(home, false).expect("second install");
    assert!(
        report
            .rows
            .iter()
            .filter(|r| r.scope == "canonical")
            .all(|r| r.action == "up-to-date"),
        "{:?}",
        report.rows
    );
}

#[test]
fn setup_leaves_foreign_content_and_force_still_lists() {
    let tmp = sandbox_home();
    let home = tmp.path();
    setup::install(home, false).expect("install");
    let canonical = setup::canonical_dir(home);
    let foreign = canonical.join("craftsman-init/EXTRA.md");
    std::fs::write(&foreign, "user content\n").expect("seed foreign");

    let report = setup::install(home, false).expect("re-install");
    let init_row = row(&report, "canonical", "craftsman-init");
    assert_eq!(init_row, "left", "modified tree must be left");
    assert!(foreign.is_file(), "foreign file untouched");

    let report = setup::install(home, true).expect("forced install");
    assert_eq!(
        row(&report, "canonical", "craftsman-init"),
        "replaced",
        "--force replaces and still lists"
    );
    assert!(
        !foreign.exists(),
        "forced replace restored the payload tree"
    );
}

#[test]
fn setup_remove_mirrors_the_same_proofs() {
    let tmp = sandbox_home();
    let home = tmp.path();
    setup::install(home, false).expect("install");
    let canonical = setup::canonical_dir(home);
    std::fs::write(canonical.join("craftsman-init/EXTRA.md"), "keep me\n").expect("modify");

    let report = setup::remove(home, false).expect("remove");
    assert!(
        canonical.join("craftsman-init").is_dir(),
        "modified tree left in place"
    );
    assert!(
        !canonical.join("craftsman-plan").exists(),
        "attributable tree removed"
    );
    assert!(
        !home.join(".claude/skills/craftsman-plan").is_symlink(),
        "agent link removed"
    );
    assert_eq!(row(&report, "canonical", "craftsman-init"), "left");

    let report = setup::remove(home, true).expect("forced remove");
    assert!(!canonical.join("craftsman-init").exists());
    assert_eq!(row(&report, "canonical", "craftsman-init"), "removed");
}

#[test]
fn embedded_conventions_copies_are_byte_identical() {
    let canonical =
        setup::embedded_file("craftsman-conventions.md").expect("canonical conventions embedded");
    let skills = setup::payload_skills().expect("six skills embedded");
    assert_eq!(skills.len(), 6);
    for skill in skills {
        let name = setup::skill_name(skill);
        let copy = setup::embedded_file(&format!("{name}/references/craftsman-conventions.md"))
            .unwrap_or_else(|| panic!("{name} lacks references/craftsman-conventions.md"));
        assert_eq!(
            hash(canonical),
            hash(copy),
            "{name}'s conventions copy drifted from the canonical — \
             refresh it byte-identically before shipping"
        );
        assert!(
            setup::payload_files(skill)
                .iter()
                .any(|(rel, _)| rel == "SKILL.md"),
            "{name} lacks SKILL.md"
        );
    }
}

fn hash(bytes: &[u8]) -> String {
    setup::digest_entries(&[("c".to_owned(), bytes.to_vec())]).expect("digest")
}

fn row(report: &setup::Report, scope: &str, skill: &str) -> &'static str {
    report
        .rows
        .iter()
        .find(|r| r.scope == scope && r.skill == skill)
        .unwrap_or_else(|| panic!("no row for {scope}/{skill}"))
        .action
}

/// Root-cause test for the craftsman-web ledger finding 1: `craftsman init
/// --stack typescript` without `--spec` scaffolded `SPEC.md`, which the
/// cucumber-js runner never discovers (`features/**/*.feature`) — the very
/// first verify exits 4 with 0 scenarios. The default must be a feature
/// file the runner actually reads.
#[test]
fn init_typescript_default_spec_is_a_discoverable_feature_file() {
    let tmp = repo();
    let output = Command::new(env!("CARGO_BIN_EXE_craftsman"))
        .args(["init", "--name", "web", "--stack", "typescript"])
        .current_dir(tmp.path())
        .output()
        .expect("init runs");
    assert!(output.status.success(), "{output:?}");

    assert!(
        tmp.path().join("features/web.feature").is_file(),
        "typescript scaffold must place the spec where cucumber-js discovers it"
    );
    assert!(
        !tmp.path().join("SPEC.md").exists(),
        "no orphan markdown spec the runner would silently ignore"
    );
    let config = std::fs::read_to_string(tmp.path().join("craftsman.toml")).expect("config");
    assert!(
        config.contains("spec = \"features/web.feature\""),
        "config must point at the feature file: {config}"
    );
}

/// GAP-R01 pin: `adopt --start-phase 2` records a baseline for every gate
/// in baseline mode — the Phase 2 "hold the line" move, end to end over a
/// real repo with a seeded health finding.
#[test]
fn adopt_phase_2_records_baselines_for_baseline_mode_gates() {
    let tmp = repo();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).expect("mkdirs");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn inherited() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    let d = 4;\n    let e = 5;\n    let _ = a + b + c + d + e;\n}\n",
    )
    .expect("seed source");
    let add = Command::new("git")
        .args(["add", "-A"])
        .current_dir(root)
        .status()
        .expect("git add");
    assert!(add.success());

    adopt::start_phase(root, 0).expect("phase 0 starts");
    adopt::complete_phase(root, 0).expect("phase 0 completes");
    adopt::start_phase(root, 1).expect("phase 1 starts (writes the config)");
    adopt::complete_phase(root, 1).expect("phase 1 completes");

    // The skill's move between phases 1 and 2: flip gates to baseline.
    let config_path = root.join("craftsman.toml");
    let mut config = std::fs::read_to_string(&config_path).expect("config");
    config.push_str("health = \"baseline\"\n\n[health]\nmax-function-lines = 5\n");
    std::fs::write(&config_path, config).expect("flip health to baseline");

    let report = adopt::start_phase(root, 2).expect("phase 2 records baselines");
    assert!(
        report
            .actions
            .iter()
            .any(|a| a.contains("gate health: baseline recorded — 1 finding(s)")),
        "phase 2 must announce the recorded debt: {:?}",
        report.actions
    );
    let baseline =
        std::fs::read_to_string(root.join(".craftsman/baselines/health.json")).expect("snapshot");
    assert!(
        baseline.contains("max-function-lines"),
        "the snapshot must carry the seeded finding: {baseline}"
    );
}

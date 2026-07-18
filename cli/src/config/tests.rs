//! The config test suite: the shared documented-example fixture plus
//! loading/validation tests (settings assertions live in `settings`).

use super::*;

/// The documented full-surface example config, shared by the mod and
/// settings test suites (one copy — the duplication gate watches).
pub const DOCUMENTED_EXAMPLE: &str = r#"
            [project]
            name = "acme-app"
            stacks = ["rust", "typescript"]
            spec = "SPEC.md"
            plan = "PLAN.md"
            cli-version = "0.4"

            [verify.rust]
            runner = "cucumber-rs"
            runner-target = "spec"
            cwd = "cli"

            [verify.python]
            runner = "pytest-bdd"
            tests-dir = "tests"
            cwd = "backend"

            [verify.typescript]
            runner = "cucumber-js"
            cwd = "web"

            [gates]
            verify = "strict"
            lint = "baseline"
            arch = "strict"
            security = "baseline"
            health = "baseline"
            mutate = "strict"
            a11y = "strict"
            visual = "off"

            [gates.tools]
            swiftlint = "0.57.0"
            gitleaks = "8.24.0"

            [budgets]
            perf.p95_ms = 200
            tokens.agents-md-lines = 100

            [docs]
            cache = ".craftsman/docs"

            [ledger]
            co-author = "Claude Fable 5 <noreply@anthropic.com>"

            [health]
            max-function-lines = 80
            dup-window = 10

            [mutate]
            min-score = 75.0

            [arch]
            deny = ["src/domain -> src/infra"]

            [perf]
            lighthouse-config = "lighthouserc.json"

            [a11y]
            test-glob = "e2e/a11y"

            [visual]
            test-glob = "e2e/visual"
            "#;

pub fn parse_documented_example() -> Config {
    Config::from_toml(DOCUMENTED_EXAMPLE, std::path::Path::new("craftsman.toml"))
        .expect("documented example must parse")
}

fn parse(text: &str) -> Result<Config, ConfigError> {
    Config::from_toml(text, Path::new("craftsman.toml"))
}

const MINIMAL: &str = r#"
    [project]
    name = "demo"
    stacks = ["rust"]
"#;

#[test]
fn accepts_minimal_config_with_defaults() {
    let c = parse(MINIMAL).expect("minimal config must parse");
    assert_eq!(c.project.name, "demo");
    assert_eq!(c.project.spec, "SPEC.md");
    assert_eq!(c.project.plan, "PLAN.md");
    assert_eq!(c.gates.verify, None);
}

#[test]
fn accepts_the_documented_field_set() {
    let c = parse_documented_example();
    assert_eq!(c.project.name, "acme-app");
    assert_eq!(c.gates.verify, Some(GateMode::Strict));
    assert_eq!(c.gates.lint, Some(GateMode::Baseline));
    assert_eq!(c.gates.tools["gitleaks"], "8.24.0");
    assert_eq!(c.docs.cache.as_deref(), Some(".craftsman/docs"));
    assert_eq!(
        c.ledger.co_author.as_deref(),
        Some("Claude Fable 5 <noreply@anthropic.com>")
    );
}

#[test]
fn rejects_unknown_health_key_and_an_empty_a11y_section() {
    let err = parse(&format!("{MINIMAL}\n[health]\nmax-vibes = 3\n"))
        .expect_err("unknown health key must be rejected");
    assert!(matches!(err, ConfigError::Parse { .. }), "{err}");
    let err = parse(&format!("{MINIMAL}\n[a11y]\n")).expect_err("an empty [a11y] must be rejected");
    assert!(matches!(err, ConfigError::A11yConfig { .. }), "{err}");
}

#[test]
fn a11y_paths_are_mutually_exclusive_and_complete() {
    // The apple pair alone is valid (destination optional).
    let c = parse(&format!(
        "{MINIMAL}\n[a11y]\nscheme = \"App\"\nui-test-target = \"AppUITests\"\n"
    ))
    .expect("apple path parses");
    let a11y = c.a11y.expect("[a11y] present");
    assert_eq!(a11y.scheme.as_deref(), Some("App"));
    assert_eq!(a11y.ui_test_target.as_deref(), Some("AppUITests"));
    assert!(a11y.destination.is_none());

    // Both paths set → rejected.
    let err = parse(&format!(
        "{MINIMAL}\n[a11y]\ntest-glob = \"e2e/a11y\"\nscheme = \"App\"\n\
         ui-test-target = \"AppUITests\"\n"
    ))
    .expect_err("both paths must be rejected");
    assert!(matches!(err, ConfigError::A11yConfig { .. }), "{err}");

    // An incomplete apple pair → rejected.
    let err = parse(&format!("{MINIMAL}\n[a11y]\nscheme = \"App\"\n"))
        .expect_err("scheme without ui-test-target must be rejected");
    assert!(matches!(err, ConfigError::A11yConfig { .. }), "{err}");
}

#[test]
fn rejects_the_pre_batch_4_flat_verify_keys() {
    // Clean break (Batch 4): `[verify]` holds per-stack tables only.
    let err = parse(&format!(
        "{MINIMAL}\n[verify]\nrunner = \"cucumber-rs\"\ncwd = \"cli\"\n"
    ))
    .expect_err("flat [verify] keys must be rejected");
    assert!(matches!(err, ConfigError::Parse { .. }), "{err}");
}

#[test]
fn rejects_unknown_project_field() {
    let err = parse(
        r#"
        [project]
        name = "demo"
        stacks = ["rust"]
        speling = "SPEC.md"
        "#,
    )
    .expect_err("unknown field must be rejected");
    assert!(matches!(err, ConfigError::Parse { .. }), "{err}");
}

#[test]
fn rejects_unknown_gate_name() {
    let err = parse(&format!("{MINIMAL}\n[gates]\nvibes = \"strict\"\n"))
        .expect_err("unknown gate must be rejected");
    assert!(matches!(err, ConfigError::Parse { .. }), "{err}");
}

#[test]
fn rejects_unknown_gate_mode() {
    let err = parse(&format!("{MINIMAL}\n[gates]\nlint = \"advisory\"\n"))
        .expect_err("unknown mode must be rejected");
    assert!(matches!(err, ConfigError::Parse { .. }), "{err}");
}

#[test]
fn rejects_verify_gate_weaker_than_strict() {
    for weaker in ["baseline", "off"] {
        let err = parse(&format!("{MINIMAL}\n[gates]\nverify = \"{weaker}\"\n"))
            .expect_err("non-strict verify must be rejected");
        assert!(matches!(err, ConfigError::VerifyNotStrict { .. }), "{err}");
    }
}

#[test]
fn accepts_strict_verify_gate() {
    parse(&format!("{MINIMAL}\n[gates]\nverify = \"strict\"\n"))
        .expect("strict verify is the only accepted verify mode");
}

#[test]
fn loads_from_nearest_ancestor() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join(FILE_NAME), MINIMAL).expect("write config");
    let nested = tmp.path().join("a/b");
    std::fs::create_dir_all(&nested).expect("mkdirs");
    let loaded = Config::load(&nested).expect("must find ancestor config");
    assert_eq!(loaded.config.project.name, "demo");
    assert_eq!(
        loaded.root.canonicalize().expect("canon"),
        tmp.path().canonicalize().expect("canon")
    );
}

#[test]
fn missing_config_is_not_found() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let err = Config::load(tmp.path()).expect_err("no config anywhere");
    assert!(matches!(err, ConfigError::NotFound { .. }), "{err}");
}

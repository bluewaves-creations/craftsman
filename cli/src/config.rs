//! `craftsman.toml` — the committed contract between human, agent, CLI, and CI.
//!
//! Schema per `docs/design/2026-07-17-cli-surface-design.md`. Unknown fields
//! are rejected (`deny_unknown_fields`) so config typos fail loudly; the one
//! deliberate exception is `[budgets]`, which is an open table by design
//! (budget keys are gate-specific and grow per stack).

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

/// The config file name searched for in the working directory and ancestors.
pub const FILE_NAME: &str = "craftsman.toml";

/// Errors loading or validating `craftsman.toml`. Exit code 3 territory.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("no {FILE_NAME} found in {start} or any ancestor directory")]
    NotFound { start: PathBuf },
    #[error("failed to read {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid {FILE_NAME} at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error(
        "[gates] verify is \"{found}\" but verify is always strict — \
         baselines never apply to the spec"
    )]
    VerifyNotStrict { found: GateMode },
}

/// Per-gate enforcement mode. Absent gate = off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GateMode {
    Off,
    Baseline,
    Strict,
}

impl fmt::Display for GateMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Off => "off",
            Self::Baseline => "baseline",
            Self::Strict => "strict",
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    pub project: Project,
    #[serde(default)]
    pub verify: Verify,
    #[serde(default)]
    pub gates: Gates,
    /// Open by design: budget keys are gate- and stack-specific.
    #[serde(default)]
    pub budgets: toml::Table,
    #[serde(default)]
    pub docs: Docs,
    #[serde(default)]
    pub adr: Adr,
    #[serde(default)]
    pub ledger: Ledger,
    /// `[health]` — thresholds for the health gate (ADR-004: gate settings
    /// are top-level tables like `[verify]`, because `[gates] health =
    /// "baseline"` already claims the `gates.health` TOML key).
    #[serde(default)]
    pub health: Health,
    /// `[mutate]` — mutation-testing settings.
    #[serde(default)]
    pub mutate: Mutate,
    /// `[arch]` — dependency-direction fitness rules.
    #[serde(default)]
    pub arch: Arch,
    /// `[perf]` — absent = the perf gate refuses to run (exit 3).
    pub perf: Option<Perf>,
    /// `[a11y]` — absent = the a11y gate refuses to run (exit 3).
    pub a11y: Option<A11y>,
    /// `[visual]` — absent = the visual gate refuses to run (exit 3).
    pub visual: Option<Visual>,
}

/// `[health]` thresholds. Defaults per the production-grade research doc:
/// function/file size, complexity, and duplication are the evidence-backed
/// entropy metrics.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Health {
    /// Max lines per function (default 60).
    pub max_function_lines: Option<usize>,
    /// Max lines per file (default 400) — health's metric, not arch's
    /// (ADR-004 corrects the design-doc sketch).
    pub max_file_lines: Option<usize>,
    /// Max cyclomatic-complexity approximation per function (default 12).
    pub max_complexity: Option<usize>,
    /// Duplicate-block window in normalized lines (default 12).
    pub dup_window: Option<usize>,
}

impl Health {
    #[must_use]
    pub fn max_function_lines(&self) -> usize {
        self.max_function_lines.unwrap_or(60)
    }
    #[must_use]
    pub fn max_file_lines(&self) -> usize {
        self.max_file_lines.unwrap_or(400)
    }
    #[must_use]
    pub fn max_complexity(&self) -> usize {
        self.max_complexity.unwrap_or(12)
    }
    #[must_use]
    pub fn dup_window(&self) -> usize {
        self.dup_window.unwrap_or(12)
    }
}

/// `[mutate]` settings.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Mutate {
    /// Minimum mutation score (percent, 0–100) on changed lines
    /// (default 60).
    pub min_score: Option<f64>,
}

impl Mutate {
    #[must_use]
    pub fn min_score(&self) -> f64 {
        self.min_score.unwrap_or(60.0)
    }
}

/// `[arch]` — fitness rules v1: dependency direction only.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Arch {
    /// `"A -> B"` rules: a file under path prefix A (stack-root-relative)
    /// must not import anything resolving under prefix B.
    #[serde(default)]
    pub deny: Vec<String>,
}

/// `[perf]` — exactly one runner: Lighthouse CI (`lighthouse-config`) or
/// k6 (`k6-script`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Perf {
    /// Path to a lighthouserc config for `lhci autorun`.
    pub lighthouse_config: Option<String>,
    /// Path to a k6 script with thresholds; runs via the pinned k6 binary.
    pub k6_script: Option<String>,
}

/// `[a11y]` — Playwright test filter for axe-based specs (user-land specs).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct A11y {
    /// Glob/filter passed to `playwright test` selecting the a11y specs.
    pub test_glob: String,
}

/// `[visual]` — Playwright test filter for screenshot-comparison specs.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Visual {
    /// Glob/filter passed to `playwright test` selecting the visual specs.
    pub test_glob: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Project {
    pub name: String,
    /// `swift-apple | swift | python | typescript | rust | bash`.
    pub stacks: Vec<String>,
    #[serde(default = "default_spec")]
    pub spec: String,
    #[serde(default = "default_plan")]
    pub plan: String,
    pub cli_version: Option<String>,
}

fn default_spec() -> String {
    "SPEC.md".to_owned()
}

fn default_plan() -> String {
    "PLAN.md".to_owned()
}

/// `[verify]` is a table of per-stack tables.
///
/// `[verify.rust]`, `[verify.python]`, `[verify.typescript]`,
/// `[verify.swift]`, `[verify.bash]` — a clean break from the Batch 2/3
/// flat keys, made while nothing external consumed them (Batch 4; swift and
/// bash added in Batch 5). Each stack listed in `[project] stacks` reads its
/// own section; an absent section means all defaults.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Verify {
    pub rust: Option<VerifyStack>,
    pub python: Option<VerifyStack>,
    pub typescript: Option<VerifyStack>,
    pub swift: Option<VerifyStack>,
    pub bash: Option<VerifyStack>,
}

impl Verify {
    /// The section for a stack name, when configured. Unknown names return
    /// `None`; the verify dispatcher owns rejecting unknown stacks loudly.
    #[must_use]
    pub fn stack(&self, name: &str) -> Option<&VerifyStack> {
        match name {
            "rust" => self.rust.as_ref(),
            "python" => self.python.as_ref(),
            "typescript" => self.typescript.as_ref(),
            "swift" => self.swift.as_ref(),
            "bash" => self.bash.as_ref(),
            _ => None,
        }
    }
}

/// One stack's verify settings. All optional — adapters own the defaults.
/// The field set is shared across stacks; the dispatcher validates that a
/// configured `runner` is one the stack actually supports.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct VerifyStack {
    /// Per-stack default overridable; `cucumber-rs` (rust), `pytest-bdd`
    /// (python), `cucumber-js` (typescript).
    pub runner: Option<String>,
    /// Cargo integration-test target the cucumber-rs harness lives in
    /// (`cargo test --test <runner-target>`). Default: `spec`. Rust only.
    pub runner_target: Option<String>,
    /// Directory containing the runnable project, relative to the config
    /// root, when the code does not live at the root (e.g. `cli/` here).
    pub cwd: Option<String>,
    /// Directory pytest collects from, relative to the stack `cwd`.
    /// Default: `tests`. Python only.
    pub tests_dir: Option<String>,
    /// `SwiftPM` package root, relative to the config root. Default: the
    /// stack `cwd` (or the root). Swift only.
    pub package_dir: Option<String>,
    /// Test-target source directory relative to `package-dir` (its last
    /// path component is the `SwiftPM` test target name, per convention).
    /// Default: the single directory under `<package-dir>/Tests/`.
    /// Swift only.
    pub swift_tests_dir: Option<String>,
    /// Directory holding the generated `.bats` file, relative to the stack
    /// `cwd`. Default: `tests`. Bash only.
    pub bats_dir: Option<String>,
    /// xcodebuild variant (swift-apple): presence of `scheme` selects
    /// `xcodebuild test` over `swift test`. Not yet supported — verify
    /// errors clearly rather than half-running (Batch 5 honest-undone).
    pub scheme: Option<String>,
    pub destination: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Gates {
    pub verify: Option<GateMode>,
    pub lint: Option<GateMode>,
    pub arch: Option<GateMode>,
    pub security: Option<GateMode>,
    pub health: Option<GateMode>,
    pub mutate: Option<GateMode>,
    pub perf: Option<GateMode>,
    pub a11y: Option<GateMode>,
    pub visual: Option<GateMode>,
    /// Minimum severity at which a security finding blocks
    /// (`info|low|medium|high|critical`). Default: `high` — HIGH and
    /// CRITICAL findings fail the gate.
    pub security_threshold: Option<crate::gates::Severity>,
    /// Version pins; adapters install hermetically (Batch 6a).
    #[serde(default)]
    pub tools: BTreeMap<String, String>,
}

impl Gates {
    /// Every gate in check-all orchestration order (Batch 6b): verify →
    /// lint → arch → security → health → mutate → perf → a11y → visual.
    /// `None` = off. Cheap static gates run before slow/runtime ones.
    #[must_use]
    pub const fn by_name(&self) -> [(&'static str, Option<GateMode>); 9] {
        [
            ("verify", self.verify),
            ("lint", self.lint),
            ("arch", self.arch),
            ("security", self.security),
            ("health", self.health),
            ("mutate", self.mutate),
            ("perf", self.perf),
            ("a11y", self.a11y),
            ("visual", self.visual),
        ]
    }

    /// The mode of one gate by name; `None` for unknown names or absent
    /// (= off) gates.
    #[must_use]
    pub fn mode(&self, name: &str) -> Option<GateMode> {
        self.by_name()
            .into_iter()
            .find(|(gate, _)| *gate == name)
            .and_then(|(_, mode)| mode)
    }
}

/// `[docs]` — the documentation pipeline (Batch 7). Sources are declared
/// via `craftsman docs add` into `.craftsman/docs/manifest.json`; the
/// AGENTS.md Documentation Sources table stays human-owned.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Docs {
    /// Cache directory, project-root-relative (default `.craftsman/docs`).
    pub cache: Option<String>,
    /// Max pages fetched per library on `docs sync` (default 200) —
    /// llms.txt indexes can list thousands of pages.
    pub max_pages: Option<usize>,
}

impl Docs {
    #[must_use]
    pub fn cache_dir(&self) -> &str {
        self.cache.as_deref().unwrap_or(".craftsman/docs")
    }
    #[must_use]
    pub fn max_pages(&self) -> usize {
        self.max_pages.unwrap_or(200)
    }
}

/// `[adr]` — decision-ledger settings (Batch 7).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Adr {
    /// Directory holding the ADR files (default `decisions`).
    pub dir: Option<String>,
    /// `adr stale` flags an active ADR once more than this many commits
    /// touched its cited files after the ADR's last commit (default 10).
    pub stale_commits: Option<u64>,
}

impl Adr {
    #[must_use]
    pub fn dir(&self) -> &str {
        self.dir.as_deref().unwrap_or("decisions")
    }
    #[must_use]
    pub fn stale_commits(&self) -> u64 {
        self.stale_commits.unwrap_or(10)
    }
}

/// `craftsman commit` settings. Committed config, not environment: the
/// co-author attribution is part of the project contract and reviewable
/// like the rest of `craftsman.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Ledger {
    /// `Name <email>` written as a `Co-Authored-By:` trailer on every
    /// ledger commit; omit the key to omit the trailer.
    pub co_author: Option<String>,
}

/// A successfully loaded config plus where it was found.
#[derive(Debug)]
pub struct Loaded {
    pub config: Config,
    /// Directory containing `craftsman.toml` — the project root.
    pub root: PathBuf,
}

impl Config {
    /// Parse and validate a config from TOML text.
    ///
    /// # Errors
    /// [`ConfigError::Parse`] on invalid TOML or unknown fields;
    /// [`ConfigError::VerifyNotStrict`] if `[gates] verify` is weaker than
    /// `strict`.
    pub fn from_toml(text: &str, path: &Path) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
        config.validate()?;
        Ok(config)
    }

    /// Load `craftsman.toml` from `start` or its nearest ancestor.
    ///
    /// # Errors
    /// [`ConfigError::NotFound`] when no ancestor holds a `craftsman.toml`;
    /// otherwise the read/parse/validation errors of [`Config::from_toml`].
    pub fn load(start: &Path) -> Result<Loaded, ConfigError> {
        let mut dir = Some(start);
        while let Some(d) = dir {
            let candidate = d.join(FILE_NAME);
            if candidate.is_file() {
                let text =
                    std::fs::read_to_string(&candidate).map_err(|source| ConfigError::Read {
                        path: candidate.clone(),
                        source,
                    })?;
                let config = Self::from_toml(&text, &candidate)?;
                return Ok(Loaded {
                    config,
                    root: d.to_path_buf(),
                });
            }
            dir = d.parent();
        }
        Err(ConfigError::NotFound {
            start: start.to_path_buf(),
        })
    }

    const fn validate(&self) -> Result<(), ConfigError> {
        match self.gates.verify {
            Some(GateMode::Strict) | None => Ok(()),
            Some(found) => Err(ConfigError::VerifyNotStrict { found }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let c = parse(
            r#"
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
            "#,
        )
        .expect("documented example must parse");
        assert_eq!(c.health.max_function_lines(), 80);
        assert_eq!(c.health.max_file_lines(), 400, "unset key keeps default");
        assert_eq!(c.health.dup_window(), 10);
        assert!((c.mutate.min_score() - 75.0).abs() < f64::EPSILON);
        assert_eq!(c.arch.deny, vec!["src/domain -> src/infra".to_owned()]);
        assert_eq!(
            c.perf.as_ref().and_then(|p| p.lighthouse_config.as_deref()),
            Some("lighthouserc.json")
        );
        assert_eq!(
            c.a11y.as_ref().map(|a| a.test_glob.as_str()),
            Some("e2e/a11y")
        );
        assert_eq!(
            c.visual.as_ref().map(|v| v.test_glob.as_str()),
            Some("e2e/visual")
        );
        assert_eq!(c.gates.verify, Some(GateMode::Strict));
        assert_eq!(c.gates.lint, Some(GateMode::Baseline));
        assert_eq!(c.gates.tools["gitleaks"], "8.24.0");
        let rust = c.verify.stack("rust").expect("[verify.rust] present");
        assert_eq!(rust.runner_target.as_deref(), Some("spec"));
        assert_eq!(rust.cwd.as_deref(), Some("cli"));
        let python = c.verify.stack("python").expect("[verify.python] present");
        assert_eq!(python.tests_dir.as_deref(), Some("tests"));
        assert!(c.verify.stack("typescript").is_some());
        assert!(c.verify.stack("cobol").is_none());
        assert_eq!(c.docs.cache.as_deref(), Some(".craftsman/docs"));
        assert_eq!(
            c.ledger.co_author.as_deref(),
            Some("Claude Fable 5 <noreply@anthropic.com>")
        );
    }

    #[test]
    fn gate_settings_default_sanely_when_absent() {
        let c = parse(MINIMAL).expect("minimal config must parse");
        assert_eq!(c.health.max_function_lines(), 60);
        assert_eq!(c.health.max_file_lines(), 400);
        assert_eq!(c.health.max_complexity(), 12);
        assert_eq!(c.health.dup_window(), 12);
        assert!((c.mutate.min_score() - 60.0).abs() < f64::EPSILON);
        assert!(c.arch.deny.is_empty());
        assert!(c.perf.is_none(), "absent [perf] means not configured");
        assert!(c.a11y.is_none());
        assert!(c.visual.is_none());
    }

    #[test]
    fn rejects_unknown_health_key_and_a11y_without_glob() {
        let err = parse(&format!("{MINIMAL}\n[health]\nmax-vibes = 3\n"))
            .expect_err("unknown health key must be rejected");
        assert!(matches!(err, ConfigError::Parse { .. }), "{err}");
        let err = parse(&format!("{MINIMAL}\n[a11y]\n"))
            .expect_err("[a11y] without test-glob must be rejected");
        assert!(matches!(err, ConfigError::Parse { .. }), "{err}");
    }

    #[test]
    fn ledger_co_author_defaults_to_absent() {
        let c = parse(MINIMAL).expect("minimal config must parse");
        assert_eq!(c.ledger.co_author, None);
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
}

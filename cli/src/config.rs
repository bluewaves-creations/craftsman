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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
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
    pub ledger: Ledger,
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
    /// Version pins; adapters install hermetically (Batch 6).
    #[serde(default)]
    pub tools: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Docs {
    pub cache: Option<String>,
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
            "#,
        )
        .expect("documented example must parse");
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

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

mod settings;

pub use settings::{
    A11y, Adr, Arch, Docs, Health, Ledger, Mutate, Perf, Verify, VerifyStack, Visual,
};

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
    #[error(
        "[a11y] must configure exactly one path — test-glob (web: \
         Playwright/axe) OR scheme + ui-test-target (apple: XCUITest \
         audit) — {detail}"
    )]
    A11yConfig { detail: &'static str },
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
    /// Glob patterns (root-relative; `*` within a segment, `**` across
    /// segments) excluded from every gate's scope — findings under them
    /// are dropped and file censuses skip them (Batch 9c). For committed
    /// evidence that is not shipped code (e.g. `spikes/**`).
    #[serde(default)]
    pub exclude: Vec<String>,
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
            Some(GateMode::Strict) | None => {}
            Some(found) => return Err(ConfigError::VerifyNotStrict { found }),
        }
        if let Some(a11y) = &self.a11y {
            return Self::validate_a11y(a11y);
        }
        Ok(())
    }

    /// The `[a11y]` two-path rule: web XOR apple, each complete.
    const fn validate_a11y(a11y: &A11y) -> Result<(), ConfigError> {
        let detail = match (
            a11y.test_glob.is_some(),
            a11y.has_apple_keys(),
            a11y.scheme.is_some() && a11y.ui_test_target.is_some(),
        ) {
            (true, true, _) => "both paths are set",
            (false, false, _) => "the section is empty",
            (false, true, false) => "the apple path needs both scheme and ui-test-target",
            _ => return Ok(()),
        };
        Err(ConfigError::A11yConfig { detail })
    }
}

#[cfg(test)]
mod tests;

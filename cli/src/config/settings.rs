//! The gate/pipeline settings tables of `craftsman.toml`: `[health]`,
//! `[mutate]`, `[arch]`, `[perf]`, `[a11y]`, `[visual]`, `[docs]`,
//! `[adr]`, and `[ledger]` — data with defaults, no loading logic.

use serde::Deserialize;

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

/// `[a11y]` — exactly one path (validated).
///
/// The web path (`test-glob` → Playwright running user-land axe specs) or
/// the Apple path (`scheme` + `ui-test-target` → xcodebuild running a
/// user-land `XCUITest` that calls `performAccessibilityAudit()`;
/// `spec gen --a11y-stub` emits the template).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct A11y {
    /// Web path: glob/filter passed to `playwright test` selecting the
    /// a11y specs.
    pub test_glob: Option<String>,
    /// Apple path: the xcodebuild scheme to test.
    pub scheme: Option<String>,
    /// Apple path: the UI-test target holding the audit tests
    /// (`-only-testing:<target>`).
    pub ui_test_target: Option<String>,
    /// Apple path: xcodebuild `-destination`; defaults to
    /// `platform=macOS` (same rule as `[verify.swift]`).
    pub destination: Option<String>,
}

impl A11y {
    /// Whether any Apple-path key is set (used by validation — the two
    /// paths are mutually exclusive).
    #[must_use]
    pub const fn has_apple_keys(&self) -> bool {
        self.scheme.is_some() || self.ui_test_target.is_some() || self.destination.is_some()
    }
}

/// `[visual]` — Playwright test filter for screenshot-comparison specs.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Visual {
    /// Glob/filter passed to `playwright test` selecting the visual specs.
    pub test_glob: String,
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
    /// `xcodebuild test` over `swift test` (Batch 9a). For a `SwiftPM`
    /// package this is the synthesized package scheme (`xcodebuild -list`).
    /// Swift only.
    pub scheme: Option<String>,
    /// xcodebuild `-destination`; defaults to `platform=macOS` so
    /// simulator-less runs work. Swift only, with `scheme`.
    pub destination: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::super::tests::parse_documented_example;
    use super::super::{Config, ConfigError};
    use std::path::Path;

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::from_toml(text, Path::new("craftsman.toml"))
    }

    const MINIMAL: &str = "[project]\nname = \"demo\"\nstacks = [\"rust\"]\n";

    #[test]
    fn documented_gate_settings_parse() {
        let c = parse_documented_example();
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
            c.a11y.as_ref().and_then(|a| a.test_glob.as_deref()),
            Some("e2e/a11y")
        );
        assert_eq!(
            c.visual.as_ref().map(|v| v.test_glob.as_str()),
            Some("e2e/visual")
        );
        let rust = c.verify.stack("rust").expect("[verify.rust] present");
        assert_eq!(rust.runner_target.as_deref(), Some("spec"));
        assert_eq!(rust.cwd.as_deref(), Some("cli"));
        let python = c.verify.stack("python").expect("[verify.python] present");
        assert_eq!(python.tests_dir.as_deref(), Some("tests"));
        assert!(c.verify.stack("typescript").is_some());
        assert!(c.verify.stack("cobol").is_none());
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
    fn ledger_co_author_defaults_to_absent() {
        let c = parse(MINIMAL).expect("minimal config must parse");
        assert_eq!(c.ledger.co_author, None);
    }
}

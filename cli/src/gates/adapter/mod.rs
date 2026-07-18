//! Declarative gate-tool adapters — the trunk-*style* format settled by
//! design decision #5 and open item #2.
//!
//! Each tool is DATA (name, version pin, runner, invocation, parser,
//! success codes, baseline kind) plus a small parser function. Nothing
//! here spawns a process; `tools` resolves runners and the gate modules
//! execute.

mod lint_parsers;
mod security_parsers;

use serde_json::Value;

use super::{Finding, GateError, Severity};

/// How a tool's pinned version is obtained and run.
#[derive(Debug, Clone, Copy)]
pub enum Runner {
    /// `uvx <name>@<version>` — uv IS the hermetic runner (house rule:
    /// python tools run through uv). Zero install state; version pinned per
    /// invocation.
    Uvx,
    /// `bunx <package>@<version>` — same, for npm-registry tools.
    Bunx { package: &'static str },
    /// GitHub release artifact installed once into
    /// `~/.craftsman/tools/<name>@<version>/` — only for tools on neither
    /// registry. `asset` may hold `{version}`, `{os}`, `{arch}`.
    /// `path_fallback` allows an ambient (e.g. brew) binary when the
    /// hermetic install is absent and the download fails.
    Github {
        repo: &'static str,
        asset: &'static str,
        archive: Archive,
        /// Path of the executable inside the archive (may hold `{version}`).
        binary: &'static str,
        os_arch: OsArch,
        path_fallback: bool,
    },
    /// Comes with the ambient language toolchain (cargo) — no pin, no
    /// install; the toolchain itself is the project's pinned environment.
    Toolchain { program: &'static str },
}

/// Release artifact packaging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archive {
    TarGz,
    TarXz,
    Zip,
    /// A bare executable.
    Raw,
}

/// Per-project os/arch token vocabulary used in asset names.
#[derive(Debug, Clone, Copy)]
pub enum OsArch {
    /// `darwin`/`linux` + `arm64`/`amd64` (osv-scanner; asset names verified
    /// against the v2.4.0 release listing).
    Go,
    /// `darwin`/`linux` + `arm64`/`x64` (gitleaks; its linux x86-64 asset is
    /// `_linux_x64.tar.gz` — verified against the v8.24.0 release listing
    /// after CI run 29645438952 404'd on the assumed `amd64` token, which
    /// arm64 macs never exercise).
    GoX64,
    /// `darwin`/`linux` + `aarch64`/`x86_64` (shellcheck).
    Uname,
    /// `macos`/`linux` + `arm64`/`amd64` (k6 releases).
    K6,
    /// Asset name carries no os/arch (`SwiftLint`'s universal zip).
    None,
}

/// Which parser function normalizes the tool's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserKind {
    CargoFmt,
    CargoClippy,
    RuffJson,
    RuffFormat,
    BiomeJson,
    SwiftlintJson,
    ShellcheckJson,
    GitleaksJson,
    SemgrepJson,
    OsvJson,
    /// Output is parsed by the owning gate module (runtime gates read
    /// report files, not stdout) — never routed through [`parse`].
    External,
}

/// How `gate baseline` records this tool's existing findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineKind {
    /// The tool has a battle-tested native mechanism (`SwiftLint`
    /// `--write-baseline`, Semgrep baseline commit ref) — wrap it.
    Native,
    /// No native baseline (Ruff #1149, clippy, …) — unified craftsman
    /// snapshot of finding fingerprints.
    Snapshot,
}

/// One declarative tool adapter. Everything the gate modules need to run a
/// tool is data here; the only code per tool is its parser arm.
#[derive(Debug)]
pub struct GateTool {
    /// Tool key — also the `[gates.tools]` pin key and the finding `tool`.
    pub name: &'static str,
    /// The gate this tool contributes to.
    pub gate: &'static str,
    /// The stack that activates it (`"*"` = stack-independent).
    pub stack: &'static str,
    /// Default version pin, overridden by `[gates.tools] <name>`.
    pub default_version: &'static str,
    pub runner: Runner,
    /// Invocation template: arguments after the resolved program.
    pub base_args: &'static [&'static str],
    pub parser: ParserKind,
    /// Exit codes meaning "the tool ran; findings carry the verdict".
    /// Anything else without parseable findings is a tool failure (exit 3).
    pub success_codes: &'static [i32],
    pub baseline: BaselineKind,
    /// Whether target file paths can be appended (`--changed` narrowing).
    /// Tools without file args run in full; findings are filtered to the
    /// changed set instead (documented per adapter).
    pub accepts_files: bool,
}

/// The closed tool list (design decision #5). Lint tools first, security
/// tools after; Batch 6b appends its gates here.
///
/// `--changed` support per adapter:
/// - fmt/clippy: no file args (cargo is project-scoped) — run full, filter
///   findings to changed files.
/// - ruff / ruff-format / biome / swiftlint / shellcheck: accept file lists.
/// - gitleaks: scans git history — `--changed` never narrows it.
/// - semgrep / osv-scanner: security verdicts are whole-tree by design —
///   run full (narrowing a secret/vuln scan to a diff hides standing risk).
pub const TOOLS: &[GateTool] = &[
    GateTool {
        name: "fmt",
        gate: "lint",
        stack: "rust",
        default_version: "toolchain",
        runner: Runner::Toolchain { program: "cargo" },
        base_args: &["fmt", "--check"],
        parser: ParserKind::CargoFmt,
        success_codes: &[0, 1],
        baseline: BaselineKind::Snapshot,
        accepts_files: false,
    },
    GateTool {
        name: "clippy",
        gate: "lint",
        stack: "rust",
        default_version: "toolchain",
        runner: Runner::Toolchain { program: "cargo" },
        base_args: &["clippy", "--all-targets", "--message-format=json"],
        parser: ParserKind::CargoClippy,
        success_codes: &[0, 101],
        baseline: BaselineKind::Snapshot,
        accepts_files: false,
    },
    GateTool {
        name: "ruff",
        gate: "lint",
        stack: "python",
        default_version: "0.13.2",
        runner: Runner::Uvx,
        base_args: &["check", "--output-format", "json"],
        parser: ParserKind::RuffJson,
        success_codes: &[0, 1],
        baseline: BaselineKind::Snapshot,
        accepts_files: true,
    },
    GateTool {
        name: "ruff-format",
        gate: "lint",
        stack: "python",
        default_version: "0.13.2",
        runner: Runner::Uvx,
        base_args: &["format", "--check"],
        parser: ParserKind::RuffFormat,
        success_codes: &[0, 1],
        baseline: BaselineKind::Snapshot,
        accepts_files: true,
    },
    GateTool {
        name: "biome",
        gate: "lint",
        stack: "typescript",
        default_version: "2.2.5",
        runner: Runner::Bunx {
            package: "@biomejs/biome",
        },
        base_args: &["check", "--reporter=json"],
        parser: ParserKind::BiomeJson,
        success_codes: &[0, 1],
        baseline: BaselineKind::Snapshot,
        accepts_files: true,
    },
    GateTool {
        name: "swiftlint",
        gate: "lint",
        stack: "swift",
        default_version: "0.57.0",
        runner: Runner::Github {
            repo: "realm/SwiftLint",
            asset: "portable_swiftlint.zip",
            archive: Archive::Zip,
            binary: "swiftlint",
            os_arch: OsArch::None,
            path_fallback: false,
        },
        base_args: &["lint", "--reporter", "json"],
        parser: ParserKind::SwiftlintJson,
        success_codes: &[0, 2],
        baseline: BaselineKind::Native,
        accepts_files: true,
    },
    GateTool {
        name: "shellcheck",
        gate: "lint",
        stack: "bash",
        default_version: "0.10.0",
        runner: Runner::Github {
            repo: "koalaman/shellcheck",
            asset: "shellcheck-v{version}.{os}.{arch}.tar.xz",
            archive: Archive::TarXz,
            binary: "shellcheck-v{version}/shellcheck",
            os_arch: OsArch::Uname,
            path_fallback: true,
        },
        base_args: &["--format", "json"],
        parser: ParserKind::ShellcheckJson,
        success_codes: &[0, 1],
        baseline: BaselineKind::Snapshot,
        accepts_files: true,
    },
    GateTool {
        name: "gitleaks",
        gate: "security",
        stack: "*",
        default_version: "8.24.0",
        runner: Runner::Github {
            repo: "gitleaks/gitleaks",
            asset: "gitleaks_{version}_{os}_{arch}.tar.gz",
            archive: Archive::TarGz,
            binary: "gitleaks",
            os_arch: OsArch::GoX64,
            path_fallback: false,
        },
        base_args: &["git", "--no-banner"],
        parser: ParserKind::GitleaksJson,
        success_codes: &[0, 1],
        baseline: BaselineKind::Snapshot,
        accepts_files: false,
    },
    GateTool {
        name: "semgrep",
        // 1.130.0 (the design-doc example pin) is broken under uv's current
        // python/setuptools (missing pkg_resources) — first verified working
        // pin: 1.146.0.
        gate: "security",
        stack: "*",
        default_version: "1.146.0",
        runner: Runner::Uvx,
        base_args: &["scan", "--json", "--metrics=off", "--quiet"],
        parser: ParserKind::SemgrepJson,
        success_codes: &[0],
        baseline: BaselineKind::Native,
        accepts_files: false,
    },
    GateTool {
        // The perf gate's k6 path (runtime.rs orchestrates; base args and
        // parsing live there because the summary comes from an export
        // file, not stdout).
        name: "k6",
        gate: "perf",
        stack: "*",
        default_version: "2.1.0",
        runner: Runner::Github {
            repo: "grafana/k6",
            asset: "k6-v{version}-{os}-{arch}.zip",
            archive: Archive::Zip,
            binary: "k6-v{version}-{os}-{arch}/k6",
            os_arch: OsArch::K6,
            path_fallback: true,
        },
        base_args: &[],
        parser: ParserKind::External,
        // 0 = pass; 99 = thresholds crossed (the verdict, not a failure).
        success_codes: &[0, 99],
        baseline: BaselineKind::Snapshot,
        accepts_files: false,
    },
    GateTool {
        name: "osv-scanner",
        gate: "security",
        stack: "*",
        default_version: "2.4.0",
        runner: Runner::Github {
            repo: "google/osv-scanner",
            asset: "osv-scanner_{os}_{arch}",
            archive: Archive::Raw,
            binary: "osv-scanner",
            os_arch: OsArch::Go,
            path_fallback: false,
        },
        base_args: &["scan", "source", "--format", "json"],
        parser: ParserKind::OsvJson,
        success_codes: &[0, 1],
        baseline: BaselineKind::Snapshot,
        accepts_files: false,
    },
];

/// Look up a tool adapter by name.
#[must_use]
pub fn tool(name: &str) -> Option<&'static GateTool> {
    TOOLS.iter().find(|t| t.name == name)
}

/// Parse one tool's captured output into normalized findings. `stdout` and
/// `stderr` are passed separately because tools disagree about which stream
/// carries the report.
///
/// # Errors
/// [`GateError::Parse`] when the output is not in the tool's documented
/// format — never silently zero findings.
pub fn parse(tool: &GateTool, stdout: &str, stderr: &str) -> Result<Vec<Finding>, GateError> {
    match tool.parser {
        ParserKind::CargoFmt => Ok(lint_parsers::parse_cargo_fmt(stdout, stderr)),
        ParserKind::CargoClippy => lint_parsers::parse_cargo_clippy(stdout),
        ParserKind::RuffJson => lint_parsers::parse_ruff(stdout),
        ParserKind::RuffFormat => Ok(lint_parsers::parse_ruff_format(stdout)),
        ParserKind::BiomeJson => lint_parsers::parse_biome(stdout),
        ParserKind::SwiftlintJson => lint_parsers::parse_swiftlint(stdout),
        ParserKind::ShellcheckJson => lint_parsers::parse_shellcheck(stdout),
        ParserKind::GitleaksJson => security_parsers::parse_gitleaks(stdout),
        ParserKind::SemgrepJson => security_parsers::parse_semgrep(stdout),
        ParserKind::OsvJson => security_parsers::parse_osv(stdout),
        ParserKind::External => Err(GateError::Parse {
            tool: tool.name,
            detail: "output is parsed by the owning gate module, never here".to_owned(),
        }),
    }
}

pub(super) fn finding(
    gate: &'static str,
    tool: &'static str,
    rule: impl Into<String>,
    file: impl Into<String>,
    line: Option<u64>,
    message: impl Into<String>,
    severity: Severity,
) -> Finding {
    Finding {
        gate,
        tool,
        rule: rule.into(),
        file: file.into(),
        line,
        message: message.into(),
        severity,
    }
}

pub(super) fn json_value(tool: &'static str, text: &str) -> Result<Value, GateError> {
    serde_json::from_str(text.trim()).map_err(|e| GateError::Parse {
        tool,
        detail: format!("invalid JSON: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_table_is_internally_consistent() {
        for t in TOOLS {
            assert!(
                !t.success_codes.is_empty(),
                "{}: empty success codes",
                t.name
            );
            assert!(
                t.success_codes.contains(&0),
                "{}: 0 must be a success code",
                t.name
            );
            assert!(tool(t.name).is_some());
        }
        assert!(tool("vibes").is_none());
    }
}

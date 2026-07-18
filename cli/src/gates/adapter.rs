//! Declarative gate-tool adapters — the trunk-*style* format settled by
//! design decision #5 and open item #2.
//!
//! Each tool is DATA (name, version pin, runner, invocation, parser,
//! success codes, baseline kind) plus a small parser function. Nothing
//! here spawns a process; `tools` resolves runners and the gate modules
//! execute.

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
    /// `darwin`/`linux` + `arm64`/`amd64` (Go releases: gitleaks, osv).
    Go,
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
            os_arch: OsArch::Go,
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
        ParserKind::CargoFmt => Ok(parse_cargo_fmt(stdout, stderr)),
        ParserKind::CargoClippy => parse_cargo_clippy(stdout),
        ParserKind::RuffJson => parse_ruff(stdout),
        ParserKind::RuffFormat => Ok(parse_ruff_format(stdout)),
        ParserKind::BiomeJson => parse_biome(stdout),
        ParserKind::SwiftlintJson => parse_swiftlint(stdout),
        ParserKind::ShellcheckJson => parse_shellcheck(stdout),
        ParserKind::GitleaksJson => parse_gitleaks(stdout),
        ParserKind::SemgrepJson => parse_semgrep(stdout),
        ParserKind::OsvJson => parse_osv(stdout),
        ParserKind::External => Err(GateError::Parse {
            tool: tool.name,
            detail: "output is parsed by the owning gate module, never here".to_owned(),
        }),
    }
}

fn finding(
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

fn json_value(tool: &'static str, text: &str) -> Result<Value, GateError> {
    serde_json::from_str(text.trim()).map_err(|e| GateError::Parse {
        tool,
        detail: format!("invalid JSON: {e}"),
    })
}

/// `cargo fmt --check`: lines of `Diff in <file>:<line>:` (observed
/// rustfmt 1.8+; older toolchains write `Diff in <file> at line <line>:`).
fn parse_cargo_fmt(stdout: &str, stderr: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for line in stdout.lines().chain(stderr.lines()) {
        let Some(rest) = line.strip_prefix("Diff in ") else {
            continue;
        };
        let rest = rest.trim_end_matches(':');
        let (file, line_no) = rest.rsplit_once(" at line ").map_or_else(
            || {
                rest.rsplit_once(':')
                    .map_or((rest, None), |(f, n)| (f, n.parse::<u64>().ok()))
            },
            |(f, n)| (f, n.parse::<u64>().ok()),
        );
        findings.push(finding(
            "lint",
            "fmt",
            "rustfmt",
            file,
            line_no,
            "rustfmt would reformat this file",
            Severity::Low,
        ));
    }
    findings
}

/// `cargo clippy --message-format=json`: JSON Lines; findings are
/// `compiler-message` records with a primary span. Duplicate diagnostics
/// (lib vs test target compilations) are deduplicated.
fn parse_cargo_clippy(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let mut findings: Vec<Finding> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let doc: Value = serde_json::from_str(line).map_err(|e| GateError::Parse {
            tool: "clippy",
            detail: format!("invalid JSON line: {e}"),
        })?;
        if doc["reason"] != "compiler-message" {
            continue;
        }
        let msg = &doc["message"];
        let level = msg["level"].as_str().unwrap_or_default();
        let severity = match level {
            "warning" => Severity::Medium,
            "error" | "error: internal compiler error" => Severity::High,
            _ => continue,
        };
        let Some(span) = msg["spans"]
            .as_array()
            .and_then(|s| s.iter().find(|sp| sp["is_primary"] == true))
        else {
            continue; // summary records ("N warnings emitted") have no span
        };
        let rule = msg["code"]["code"].as_str().unwrap_or(level).to_owned();
        let file = span["file_name"].as_str().unwrap_or_default().to_owned();
        let line_no = span["line_start"].as_u64();
        let message = msg["message"].as_str().unwrap_or_default().to_owned();
        let key = format!("{rule}\x1f{file}\x1f{line_no:?}\x1f{message}");
        if seen.insert(key) {
            findings.push(finding(
                "lint", "clippy", rule, file, line_no, message, severity,
            ));
        }
    }
    Ok(findings)
}

/// `ruff check --output-format json`: an array of violations.
fn parse_ruff(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("ruff", stdout)?;
    let items = doc.as_array().ok_or_else(|| GateError::Parse {
        tool: "ruff",
        detail: "expected a top-level array".to_owned(),
    })?;
    Ok(items
        .iter()
        .map(|v| {
            finding(
                "lint",
                "ruff",
                v["code"].as_str().unwrap_or("ruff"),
                v["filename"].as_str().unwrap_or_default(),
                v["location"]["row"].as_u64(),
                v["message"].as_str().unwrap_or_default(),
                Severity::Medium,
            )
        })
        .collect())
}

/// `ruff format --check`: plain lines `Would reformat: <file>`.
fn parse_ruff_format(stdout: &str) -> Vec<Finding> {
    stdout
        .lines()
        .filter_map(|l| l.strip_prefix("Would reformat: "))
        .map(|file| {
            finding(
                "lint",
                "ruff-format",
                "ruff-format",
                file,
                None,
                "ruff format would reformat this file",
                Severity::Low,
            )
        })
        .collect()
}

/// `biome check --reporter=json`: `{"diagnostics": [...]}` with byte-span
/// locations (no line numbers without re-deriving them — left `None`).
fn parse_biome(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("biome", stdout)?;
    let diags = doc["diagnostics"]
        .as_array()
        .ok_or_else(|| GateError::Parse {
            tool: "biome",
            detail: "expected a `diagnostics` array".to_owned(),
        })?;
    Ok(diags
        .iter()
        .map(|d| {
            let severity = match d["severity"].as_str().unwrap_or_default() {
                "error" | "fatal" => Severity::High,
                "warning" => Severity::Medium,
                _ => Severity::Info,
            };
            let file = d["location"]["path"]["file"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            finding(
                "lint",
                "biome",
                d["category"].as_str().unwrap_or("biome"),
                file,
                None,
                d["description"].as_str().unwrap_or_default(),
                severity,
            )
        })
        .collect())
}

/// `swiftlint lint --reporter json`: an array of violations.
fn parse_swiftlint(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("swiftlint", stdout)?;
    let items = doc.as_array().ok_or_else(|| GateError::Parse {
        tool: "swiftlint",
        detail: "expected a top-level array".to_owned(),
    })?;
    Ok(items
        .iter()
        .map(|v| {
            let severity = match v["severity"].as_str().unwrap_or_default() {
                "Error" | "error" => Severity::High,
                _ => Severity::Medium,
            };
            finding(
                "lint",
                "swiftlint",
                v["rule_id"].as_str().unwrap_or("swiftlint"),
                v["file"].as_str().unwrap_or_default(),
                v["line"].as_u64(),
                v["reason"].as_str().unwrap_or_default(),
                severity,
            )
        })
        .collect())
}

/// `shellcheck --format json`: an array of comments.
fn parse_shellcheck(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("shellcheck", stdout)?;
    let items = doc.as_array().ok_or_else(|| GateError::Parse {
        tool: "shellcheck",
        detail: "expected a top-level array".to_owned(),
    })?;
    Ok(items
        .iter()
        .map(|v| {
            let severity = match v["level"].as_str().unwrap_or_default() {
                "error" => Severity::High,
                "warning" => Severity::Medium,
                _ => Severity::Info,
            };
            let rule = v["code"]
                .as_u64()
                .map_or_else(|| "shellcheck".to_owned(), |c| format!("SC{c}"));
            finding(
                "lint",
                "shellcheck",
                rule,
                v["file"].as_str().unwrap_or_default(),
                v["line"].as_u64(),
                v["message"].as_str().unwrap_or_default(),
                severity,
            )
        })
        .collect())
}

/// `gitleaks git --report-format json`: an array of leaks. Secrets never
/// enter the finding message — only the rule's description; the secret's
/// content contributes (hashed) to the baseline fingerprint via `message`?
/// No: the fingerprint hashes this message, so the message carries a hash
/// of the secret, not the secret.
fn parse_gitleaks(report: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("gitleaks", report)?;
    let items = doc.as_array().ok_or_else(|| GateError::Parse {
        tool: "gitleaks",
        detail: "expected a top-level array".to_owned(),
    })?;
    Ok(items
        .iter()
        .map(|v| {
            let secret_hash = super::fnv_hex(v["Secret"].as_str().unwrap_or_default());
            finding(
                "security",
                "gitleaks",
                v["RuleID"].as_str().unwrap_or("gitleaks"),
                v["File"].as_str().unwrap_or_default(),
                v["StartLine"].as_u64(),
                format!(
                    "{} [secret fnv:{secret_hash}]",
                    v["Description"].as_str().unwrap_or("secret detected")
                ),
                Severity::Critical,
            )
        })
        .collect())
}

/// `semgrep scan --json`: `{"results": [...]}`.
fn parse_semgrep(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("semgrep", stdout)?;
    let results = doc["results"].as_array().ok_or_else(|| GateError::Parse {
        tool: "semgrep",
        detail: "expected a `results` array".to_owned(),
    })?;
    Ok(results
        .iter()
        .map(|v| {
            let severity = match v["extra"]["severity"].as_str().unwrap_or_default() {
                "ERROR" => Severity::High,
                "WARNING" => Severity::Medium,
                _ => Severity::Info,
            };
            finding(
                "security",
                "semgrep",
                v["check_id"].as_str().unwrap_or("semgrep"),
                v["path"].as_str().unwrap_or_default(),
                v["start"]["line"].as_u64(),
                v["extra"]["message"]
                    .as_str()
                    .unwrap_or_default()
                    .lines()
                    .next()
                    .unwrap_or_default(),
                severity,
            )
        })
        .collect())
}

/// `osv-scanner scan source --format json`: `{"results": [{source,
/// packages: [{package, vulnerabilities}]}]}`. Severity comes from
/// `database_specific.severity` when present; unknown = High (conservative:
/// an unrated vulnerability is not a pass).
fn parse_osv(stdout: &str) -> Result<Vec<Finding>, GateError> {
    let doc = json_value("osv-scanner", stdout)?;
    let results = match doc["results"].as_array() {
        Some(r) => r,
        // A clean scan may emit `{"results":[]}` or omit results entirely.
        None if doc.is_object() => return Ok(Vec::new()),
        None => {
            return Err(GateError::Parse {
                tool: "osv-scanner",
                detail: "expected a `results` array".to_owned(),
            });
        }
    };
    let mut findings = Vec::new();
    for source in results {
        let file = source["source"]["path"].as_str().unwrap_or_default();
        for pkg in source["packages"].as_array().unwrap_or(&Vec::new()) {
            let name = pkg["package"]["name"].as_str().unwrap_or("?");
            let version = pkg["package"]["version"].as_str().unwrap_or("?");
            for vuln in pkg["vulnerabilities"].as_array().unwrap_or(&Vec::new()) {
                let id = vuln["id"].as_str().unwrap_or("OSV");
                let severity = match vuln["database_specific"]["severity"]
                    .as_str()
                    .unwrap_or_default()
                    .to_ascii_uppercase()
                    .as_str()
                {
                    "CRITICAL" => Severity::Critical,
                    "MODERATE" | "MEDIUM" => Severity::Medium,
                    "LOW" => Severity::Low,
                    // "HIGH" and anything unrated: an unknown severity is
                    // not a pass.
                    _ => Severity::High,
                };
                findings.push(finding(
                    "security",
                    "osv-scanner",
                    id,
                    file,
                    None,
                    format!(
                        "{name}@{version}: {}",
                        vuln["summary"].as_str().unwrap_or("known vulnerability")
                    ),
                    severity,
                ));
            }
        }
    }
    Ok(findings)
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

    #[test]
    fn cargo_fmt_diff_lines_become_findings() {
        let out = "Diff in /repo/cli/src/lib.rs:14:\n-bad\n+good\nDiff in /repo/cli/src/main.rs at line 3:\n";
        let f = parse_cargo_fmt(out, "");
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].file, "/repo/cli/src/lib.rs");
        assert_eq!(f[0].line, Some(14));
        assert_eq!(f[1].file, "/repo/cli/src/main.rs");
        assert_eq!(f[1].line, Some(3));
        assert!(parse_cargo_fmt("", "").is_empty());
    }

    #[test]
    fn clippy_json_lines_become_deduplicated_findings() {
        let line = r#"{"reason":"compiler-message","message":{"code":{"code":"clippy::needless_return"},"level":"warning","message":"unneeded `return` statement","spans":[{"is_primary":true,"file_name":"src/lib.rs","line_start":7}]}}"#;
        let summary = r#"{"reason":"compiler-message","message":{"code":null,"level":"warning","message":"1 warning emitted","spans":[]}}"#;
        let out = format!(
            "{line}\n{line}\n{summary}\n{{\"reason\":\"build-finished\",\"success\":true}}"
        );
        let f = parse_cargo_clippy(&out).expect("parses");
        assert_eq!(f.len(), 1, "duplicates and span-less summaries drop");
        assert_eq!(f[0].rule, "clippy::needless_return");
        assert_eq!(f[0].line, Some(7));
        assert_eq!(f[0].severity, Severity::Medium);
    }

    #[test]
    fn ruff_json_and_format_lines_parse() {
        let checked = r#"[{"code":"F401","filename":"app.py","location":{"row":1,"column":8},"message":"`os` imported but unused"}]"#;
        let f = parse_ruff(checked).expect("parses");
        assert_eq!(f[0].rule, "F401");
        assert_eq!(f[0].line, Some(1));
        let fmt = parse_ruff_format("Would reformat: app.py\n1 file would be reformatted\n");
        assert_eq!(fmt.len(), 1);
        assert_eq!(fmt[0].file, "app.py");
    }

    #[test]
    fn biome_swiftlint_shellcheck_parse() {
        let biome = r#"{"summary":{},"diagnostics":[{"category":"lint/suspicious/noDoubleEquals","severity":"error","description":"Use === instead of ==","location":{"path":{"file":"src/a.ts"}}}]}"#;
        let f = parse_biome(biome).expect("parses");
        assert_eq!(f[0].rule, "lint/suspicious/noDoubleEquals");
        assert_eq!(f[0].severity, Severity::High);

        let swift = r#"[{"rule_id":"line_length","file":"/app/A.swift","line":12,"reason":"Line too long","severity":"Warning"}]"#;
        let f = parse_swiftlint(swift).expect("parses");
        assert_eq!(f[0].rule, "line_length");
        assert_eq!(f[0].severity, Severity::Medium);

        let sc = r#"[{"file":"run.sh","line":3,"level":"warning","code":2086,"message":"Double quote to prevent globbing."}]"#;
        let f = parse_shellcheck(sc).expect("parses");
        assert_eq!(f[0].rule, "SC2086");
        assert_eq!(f[0].line, Some(3));
    }

    #[test]
    fn security_reports_parse_and_hide_secrets() {
        let leaks = r#"[{"RuleID":"aws-access-key","File":"config/prod.env","StartLine":2,"Description":"AWS Access Key","Secret":"AKIA123","Commit":"abc"}]"#;
        let f = parse_gitleaks(leaks).expect("parses");
        assert_eq!(f[0].severity, Severity::Critical);
        assert!(!f[0].message.contains("AKIA123"), "secret must not leak");
        assert!(
            f[0].message.contains("fnv:"),
            "hash anchors the fingerprint"
        );

        let sg = r#"{"results":[{"check_id":"rust.lang.security.unsafe","path":"src/x.rs","start":{"line":9},"extra":{"message":"unsafe block\nmore","severity":"ERROR"}}],"errors":[]}"#;
        let f = parse_semgrep(sg).expect("parses");
        assert_eq!(f[0].severity, Severity::High);
        assert_eq!(f[0].message, "unsafe block");

        let osv = r#"{"results":[{"source":{"path":"/r/Cargo.lock","type":"lockfile"},"packages":[{"package":{"name":"time","version":"0.1.0","ecosystem":"crates.io"},"vulnerabilities":[{"id":"RUSTSEC-2020-0071","summary":"Segfault in time","database_specific":{"severity":"MODERATE"}}]}]}]}"#;
        let f = parse_osv(osv).expect("parses");
        assert_eq!(f[0].rule, "RUSTSEC-2020-0071");
        assert_eq!(f[0].severity, Severity::Medium);
        assert!(f[0].message.contains("time@0.1.0"));
        assert!(parse_osv("{}").expect("clean scan").is_empty());
    }
}

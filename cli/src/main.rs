//! The Craftsman Dev CLI — mechanical verification for agentic development.
//!
//! Command layer only: clap parsing, exit-code mapping, output routing.
//! All logic lives in the library modules (`thiserror`); this file is the
//! sole `anyhow` consumer per repo conventions.

use anyhow::Context as _;
use clap::{Parser, Subcommand};

use craftsman::config::Config;
use craftsman::spec::{self, Severity};

/// Exit codes are a documented contract (design doc):
/// 0 pass · 1 verification failure · 2 usage error (clap's default) ·
/// 3 orchestrator error · 4 empty selection.
const EXIT_PASS: i32 = 0;
const EXIT_VERIFICATION_FAILURE: i32 = 1;
const EXIT_ORCHESTRATOR_ERROR: i32 = 3;

/// The Craftsman Dev CLI — mechanical verification for agentic development.
///
/// Exit codes: 0 pass · 1 verification failure · 2 usage error ·
/// 3 orchestrator error · 4 empty selection.
/// Every command supports --json (JSON to stdout, human progress to stderr).
#[derive(Parser)]
#[command(name = "craftsman", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// SPEC.md engine: scenario inventory and authoring checks
    Spec {
        #[command(subcommand)]
        command: SpecCommand,
    },
}

#[derive(Subcommand)]
enum SpecCommand {
    /// Scenario inventory: every scenario with tags, line, and status.
    ///
    /// Without recorded run results every scenario reports status
    /// "unknown" — run `craftsman verify` for verdicts.
    Status {
        /// Emit the inventory as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Gherkin authoring + code-gen-compatibility lint (ADR-001 rules).
    ///
    /// Exit 1 on any error finding; warnings alone stay green.
    Lint {
        /// Emit findings as JSON on stdout
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let code = match run(&cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            EXIT_ORCHESTRATOR_ERROR
        }
    };
    std::process::exit(code);
}

fn run(cli: &Cli) -> anyhow::Result<i32> {
    match &cli.command {
        Command::Spec { command } => match command {
            SpecCommand::Status { json } => spec_status(*json),
            SpecCommand::Lint { json } => spec_lint(*json),
        },
    }
}

/// Load config + spec relative to the config root.
fn load_spec() -> anyhow::Result<(gherkin::Feature, String)> {
    let cwd = std::env::current_dir().context("cannot determine working directory")?;
    let loaded = Config::load(&cwd)?;
    let spec_rel = loaded.config.project.spec;
    let feature = spec::parse_spec(&loaded.root.join(&spec_rel))?;
    Ok((feature, spec_rel))
}

fn spec_status(json: bool) -> anyhow::Result<i32> {
    let (feature, spec_rel) = load_spec()?;
    let entries = spec::inventory(&feature);

    if json {
        let doc = serde_json::json!({
            "spec": spec_rel,
            "feature": feature.name,
            "scenarios": entries.iter().map(|e| {
                serde_json::json!({
                    "feature": e.feature,
                    "scenario": e.scenario,
                    "tags": e.tags,
                    "line": e.line,
                    "outline_rows": e.outline_rows,
                    "status": "unknown",
                })
            }).collect::<Vec<_>>(),
        });
        println!("{doc:#}");
    } else {
        println!("Feature: {} ({spec_rel})", feature.name);
        for e in &entries {
            let tags = if e.tags.is_empty() {
                String::new()
            } else {
                format!("  [@{}]", e.tags.join(" @"))
            };
            let rows = e
                .outline_rows
                .map(|n| format!("  ({n} example rows)"))
                .unwrap_or_default();
            println!("  unknown  {}  (line {}){tags}{rows}", e.scenario, e.line);
        }
        println!(
            "{} scenarios — status unknown (no run results yet; run `craftsman verify`)",
            entries.len()
        );
    }
    Ok(EXIT_PASS)
}

fn spec_lint(json: bool) -> anyhow::Result<i32> {
    let cwd = std::env::current_dir().context("cannot determine working directory")?;
    let loaded = Config::load(&cwd)?;
    let spec_path = loaded.root.join(&loaded.config.project.spec);

    // A spec that does not parse cannot be verified: report it as a lint
    // error finding (exit 1), not an orchestrator error. An unreadable spec
    // stays an orchestrator error (exit 3).
    let findings = match spec::parse_spec(&spec_path) {
        Ok(feature) => spec::lint(&feature),
        Err(err @ spec::SpecError::Read { .. }) => return Err(err.into()),
        Err(spec::SpecError::Parse { message, .. }) => vec![spec::Finding {
            severity: Severity::Error,
            rule: "parse-error",
            line: 0,
            message,
        }],
    };

    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings.len() - errors;

    if json {
        let doc = serde_json::json!({
            "spec": loaded.config.project.spec,
            "findings": findings,
            "errors": errors,
            "warnings": warnings,
        });
        println!("{doc:#}");
    } else {
        for f in &findings {
            let sev = match f.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            println!("{sev}[{}] line {}: {}", f.rule, f.line, f.message);
        }
        println!(
            "spec lint: {errors} error(s), {warnings} warning(s) in {}",
            loaded.config.project.spec
        );
    }
    Ok(if errors > 0 {
        EXIT_VERIFICATION_FAILURE
    } else {
        EXIT_PASS
    })
}

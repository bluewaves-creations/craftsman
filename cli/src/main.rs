//! The Craftsman Dev CLI — mechanical verification for agentic development.
//!
//! Command layer only: clap parsing, exit-code mapping, output routing.
//! All logic lives in the library modules (`thiserror`); this file is the
//! sole `anyhow` consumer per repo conventions.

use anyhow::Context as _;
use clap::{Parser, Subcommand};

use craftsman::config::Config;
use craftsman::doctor;
use craftsman::ledger::{self, CommitRequest, CommitType};
use craftsman::plan;
use craftsman::spec::{self, Severity};
use craftsman::verify::{self, Outcome, Selection};

/// Exit codes are a documented contract (design doc):
/// 0 pass · 1 verification failure · 2 usage error (clap's default) ·
/// 3 orchestrator error · 4 empty selection.
const EXIT_PASS: i32 = 0;
const EXIT_VERIFICATION_FAILURE: i32 = 1;
const EXIT_ORCHESTRATOR_ERROR: i32 = 3;
const EXIT_EMPTY_SELECTION: i32 = 4;

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
    /// Plan engine: keep the batch → scenario mapping honest
    Plan {
        #[command(subcommand)]
        command: PlanCommand,
    },
    /// THE gate: run SPEC.md scenarios via the stack adapter.
    ///
    /// Exit codes: 0 all passed · 1 any failed/undefined/ambiguous ·
    /// 3 tool or config error · 4 selection matched no scenarios.
    Verify {
        /// Run only the scenarios listed under `## Batch N` in the plan
        #[arg(long, conflicts_with = "scenario")]
        batch: Option<u32>,
        /// Run a single scenario by exact name
        #[arg(long)]
        scenario: Option<String>,
        /// Emit the normalized results as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Structured ledger commit — the single writer of Verified-by.
    ///
    /// Commits exactly what is already staged (stage with `git add` first;
    /// exit 3 when nothing is staged) and only after the gates are green
    /// (exit 1 on a red gate, nothing committed). The Verified-by trailer
    /// is written by the CLI alone — there is no flag to set it. Configure
    /// an optional Co-Authored-By trailer via `[ledger] co-author` in
    /// craftsman.toml.
    Commit {
        /// Conventional commit type
        #[arg(long = "type", value_enum)]
        commit_type: CommitType,
        /// Optional scope, e.g. `batch-3` → `feat(batch-3): …`
        #[arg(long)]
        scope: Option<String>,
        /// Commit subject line
        #[arg(long)]
        message: String,
        /// Body line (repeatable, in order)
        #[arg(long)]
        body: Vec<String>,
        /// Read the body from a file instead of --body lines
        #[arg(long, conflicts_with = "body")]
        body_file: Option<std::path::PathBuf>,
        /// `Scenarios:` trailer value (repeatable)
        #[arg(long)]
        scenarios: Vec<String>,
        /// `Learned:` trailer value (repeatable)
        #[arg(long)]
        learned: Vec<String>,
        /// `Rejected:` trailer value (repeatable)
        #[arg(long)]
        rejected: Vec<String>,
        /// `Ref:` trailer value (repeatable)
        #[arg(long = "ref")]
        refs: Vec<String>,
        /// `Dependency:` trailer value (repeatable)
        #[arg(long)]
        dependency: Vec<String>,
        /// Emit {committed, sha, gates} as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Prove the loop closes: config, spec, plan, tools, and a red→green
    /// verify round trip in a disposable fixture project.
    ///
    /// The fixture is cached under the system temp dir; the first run
    /// compiles its cucumber harness and may take minutes.
    Doctor {
        /// Emit per-check results as JSON on stdout
        #[arg(long)]
        json: bool,
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

#[derive(Subcommand)]
enum PlanCommand {
    /// Validate the plan's batch → scenario mapping against the spec.
    ///
    /// Errors (exit 1): a batch lists a scenario missing from the spec;
    /// a scenario is assigned to two batches. Warnings (still exit 0):
    /// spec scenarios not assigned to any batch.
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
        Command::Plan { command } => match command {
            PlanCommand::Lint { json } => plan_lint(*json),
        },
        Command::Verify {
            batch,
            scenario,
            json,
        } => verify_cmd(*batch, scenario.as_deref(), *json),
        Command::Commit {
            commit_type,
            scope,
            message,
            body,
            body_file,
            scenarios,
            learned,
            rejected,
            refs,
            dependency,
            json,
        } => {
            let body = match body_file {
                Some(path) => std::fs::read_to_string(path)
                    .with_context(|| format!("cannot read --body-file {}", path.display()))?
                    .lines()
                    .map(str::to_owned)
                    .collect(),
                None => body.clone(),
            };
            let request = CommitRequest {
                commit_type: *commit_type,
                scope: scope.clone(),
                subject: message.clone(),
                body,
                scenarios: scenarios.clone(),
                learned: learned.clone(),
                rejected: rejected.clone(),
                refs: refs.clone(),
                dependencies: dependency.clone(),
            };
            commit_cmd(&request, *json)
        }
        Command::Doctor { json } => doctor_cmd(*json),
    }
}

fn doctor_cmd(json: bool) -> anyhow::Result<i32> {
    let cwd = std::env::current_dir().context("cannot determine working directory")?;
    let checks = doctor::run(&cwd);
    let passed = checks.iter().all(|c| c.passed);

    for c in &checks {
        let mark = if c.passed { "ok  " } else { "FAIL" };
        println!("{mark}  {:<10}  {}", c.name, c.detail);
    }
    println!(
        "doctor: {}/{} checks passed",
        checks.iter().filter(|c| c.passed).count(),
        checks.len()
    );
    if json {
        let doc = serde_json::json!({ "passed": passed, "checks": checks });
        println!("{doc:#}");
    }
    Ok(if passed {
        EXIT_PASS
    } else {
        EXIT_VERIFICATION_FAILURE
    })
}

fn commit_cmd(request: &CommitRequest, json: bool) -> anyhow::Result<i32> {
    let cwd = std::env::current_dir().context("cannot determine working directory")?;
    let report = ledger::commit(&cwd, request)?;

    for g in &report.gates {
        if g.passed {
            eprintln!("gate {}: ok ({})", g.gate, g.detail);
        } else {
            eprintln!("gate {} FAILED:\n{}", g.gate, g.detail);
        }
    }
    if json {
        let doc = serde_json::json!({
            "committed": report.committed,
            "sha": report.sha,
            "subject": report.subject,
            "gates": report.gates,
        });
        println!("{doc:#}");
    }
    if report.committed {
        let sha = report.sha.as_deref().unwrap_or("");
        let short = &sha[..sha.len().min(9)];
        eprintln!("committed {short} {}", report.subject);
        Ok(EXIT_PASS)
    } else {
        let red = report
            .gates
            .iter()
            .filter(|g| !g.passed)
            .map(|g| g.gate)
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("commit refused — red gate: {red} (nothing committed)");
        Ok(EXIT_VERIFICATION_FAILURE)
    }
}

fn verify_cmd(batch: Option<u32>, scenario: Option<&str>, json: bool) -> anyhow::Result<i32> {
    let selection = match (batch, scenario) {
        (Some(n), _) => Selection::Batch(n),
        (None, Some(name)) => Selection::Scenario(name.to_owned()),
        (None, None) => Selection::All,
    };
    let cwd = std::env::current_dir().context("cannot determine working directory")?;
    let report = verify::run(&cwd, &selection)?;

    for w in &report.warnings {
        eprintln!("warning: {w}");
    }
    for section in &report.stacks {
        eprintln!("stack {}:", section.stack);
        for r in &section.results {
            let mark = match r.status {
                craftsman::verify::normalize::Status::Passed => "pass",
                craftsman::verify::normalize::Status::Skipped => "skip",
                craftsman::verify::normalize::Status::Pending => "pend",
                craftsman::verify::normalize::Status::Undefined => "unde",
                craftsman::verify::normalize::Status::Ambiguous => "ambi",
                craftsman::verify::normalize::Status::Failed => "FAIL",
            };
            eprintln!("  {mark}  {}", r.scenario);
            if let Some(failure) = &r.failure {
                for line in failure.lines() {
                    eprintln!("        {line}");
                }
            }
        }
    }
    let c = report.counts;
    eprintln!(
        "verify: {} passed, {} failed, {} undefined, {} ambiguous, {} skipped, {} pending",
        c.passed, c.failed, c.undefined, c.ambiguous, c.skipped, c.pending
    );

    if json {
        let doc = serde_json::json!({
            "gate": "verify",
            "status": report.outcome,
            "scenarios": report.counts,
            "stacks": report.stacks,
            "warnings": report.warnings,
        });
        println!("{doc:#}");
    }

    Ok(match report.outcome {
        Outcome::Passed => EXIT_PASS,
        Outcome::Failed => EXIT_VERIFICATION_FAILURE,
        Outcome::EmptySelection => {
            eprintln!("verify: selection matched no scenarios (exit 4 — never silent success)");
            EXIT_EMPTY_SELECTION
        }
    })
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

fn plan_lint(json: bool) -> anyhow::Result<i32> {
    let cwd = std::env::current_dir().context("cannot determine working directory")?;
    let loaded = Config::load(&cwd)?;
    let feature = spec::parse_spec(&loaded.root.join(&loaded.config.project.spec))?;
    let names: Vec<String> = spec::inventory(&feature)
        .into_iter()
        .map(|e| e.scenario)
        .collect();
    let plan_rel = loaded.config.project.plan;
    let batches = plan::parse_plan(&loaded.root.join(&plan_rel))?;
    let findings = plan::lint(&batches, &names);

    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings.len() - errors;
    let assigned: usize = batches.iter().map(|b| b.scenarios.len()).sum();

    if json {
        let doc = serde_json::json!({
            "plan": plan_rel,
            "spec": loaded.config.project.spec,
            "batches": batches.len(),
            "assigned": assigned,
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
            "plan lint: {errors} error(s), {warnings} warning(s) in {plan_rel} \
             ({} batches, {assigned} scenario assignments)",
            batches.len()
        );
    }
    Ok(if errors > 0 {
        EXIT_VERIFICATION_FAILURE
    } else {
        EXIT_PASS
    })
}

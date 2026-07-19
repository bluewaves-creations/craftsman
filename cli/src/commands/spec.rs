//! `craftsman spec` and `craftsman plan` — the spec engine's commands.

use clap::Subcommand;

use craftsman::plan;
use craftsman::spec::{self, Severity};

use super::{EXIT_EMPTY_SELECTION, EXIT_PASS, EXIT_VERIFICATION_FAILURE, cwd, load};

#[derive(Subcommand)]
pub enum SpecCommand {
    /// Scenario inventory with the last recorded verify verdicts: every
    /// scenario with tags, line, and status, plus a per-batch rollup from
    /// the plan.
    ///
    /// Verdicts come from .craftsman/cache/last-verify.json (written by
    /// every `craftsman verify` run); scenarios the last run did not
    /// include report "unknown", and a note flags records older than the
    /// current git HEAD.
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
        /// Lint SPEC.delta.md (next to the executed spec) instead: the
        /// authoring rules plus name collisions against the executed
        /// spec, without admitting anything to the executed set. Exit 4
        /// when no delta file exists.
        #[arg(long)]
        delta: bool,
    },
    /// Fold an approved SPEC.delta.md into the executed spec under a
    /// banner and remove the delta file — the mechanical boundary merge,
    /// so the single-writer rule covers it too.
    ///
    /// Refuses (exit 1) while the delta has lint errors; exit 4 when no
    /// delta file exists. Writes the spec, never commits: the repository
    /// head stays where it was.
    MergeDelta {
        /// Emit the merge report as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Generate runner glue from SPEC.md for the code-gen stacks
    /// (swift → Swift Testing, bash → bats).
    ///
    /// The generated runner file is fully rewritten on every run; the step
    /// stub template is written once and never overwritten, and your real
    /// step files are never touched. Exit 1 when the spec has lint errors
    /// (fix them first — run `craftsman spec lint`); exit 4 when no
    /// configured stack needs code-gen.
    Gen {
        /// Emit the written/kept file list as JSON on stdout
        #[arg(long)]
        json: bool,
        /// Emit only the write-once UI-test accessibility-audit template
        /// (AccessibilityAuditTests.swift.template at the project root) for
        /// the Apple a11y path ([a11y] scheme + ui-test-target)
        #[arg(long)]
        a11y_stub: bool,
    },
}

/// Dispatch `craftsman spec <status|lint|gen|merge-delta>`.
pub fn run(command: &SpecCommand) -> anyhow::Result<i32> {
    match command {
        SpecCommand::Status { json } => spec_status(*json),
        SpecCommand::Lint { json, delta: false } => spec_lint(*json),
        SpecCommand::Lint { json, delta: true } => super::spec_delta::spec_lint_delta(*json),
        SpecCommand::Gen { json, a11y_stub } => spec_gen(*json, *a11y_stub),
        SpecCommand::MergeDelta { json } => super::spec_delta::spec_merge_delta(*json),
    }
}

/// The recorded verdict mark for a scenario (spec status vocabulary —
/// "unknown" when the last verify run did not include it).
const fn status_mark(status: Option<craftsman::verify::normalize::Status>) -> &'static str {
    use craftsman::verify::normalize::Status;
    match status {
        None => "unknown",
        Some(Status::Passed) => "pass",
        Some(Status::Skipped) => "skip",
        Some(Status::Pending) => "pend",
        Some(Status::Undefined) => "unde",
        Some(Status::Ambiguous) => "ambi",
        Some(Status::Failed) => "FAIL",
    }
}

/// Per-batch rollup over the recorded verdicts: (green, red, unknown) —
/// red = failed/undefined/ambiguous; skipped/pending count as unknown.
fn batch_rollup(
    batch: &plan::PlanBatch,
    record: Option<&craftsman::verify::record::LastVerify>,
) -> (usize, usize, usize) {
    use craftsman::verify::normalize::Status;
    let mut tally = (0, 0, 0);
    for (_, name) in &batch.scenarios {
        match record.and_then(|r| r.scenario_status(name)) {
            Some(Status::Passed) => tally.0 += 1,
            Some(Status::Failed | Status::Undefined | Status::Ambiguous) => tally.1 += 1,
            _ => tally.2 += 1,
        }
    }
    tally
}

fn spec_status(json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let root = &loaded.root;
    let spec_rel = &loaded.config.project.spec;
    let feature = spec::parse_spec(&root.join(spec_rel))?;
    let entries = spec::inventory(&feature);
    let record = craftsman::verify::record::load(root);
    // Batches without a Scenarios list are not yet detailed — no rollup row.
    let batches: Vec<plan::PlanBatch> = plan::parse_plan(&root.join(&loaded.config.project.plan))
        .map(|all| {
            all.into_iter()
                .filter(|b| !b.scenarios.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let stale = record.as_ref().and_then(|r| r.stale(root));

    if json {
        print_spec_status_json(
            spec_rel,
            &feature.name,
            &entries,
            record.as_ref(),
            &batches,
            stale,
        );
    } else {
        print_spec_status_human(spec_rel, &feature.name, &entries, record.as_ref(), &batches);
    }
    if stale == Some(true) {
        eprintln!(
            "note: HEAD has moved since the last verify run — recorded verdicts \
             may be stale; re-run `craftsman verify`"
        );
    }
    if let Some(line) = craftsman::session::distance_line(root) {
        eprintln!("{line}");
    }
    Ok(EXIT_PASS)
}

fn print_spec_status_json(
    spec_rel: &str,
    feature: &str,
    entries: &[spec::ScenarioEntry],
    record: Option<&craftsman::verify::record::LastVerify>,
    batches: &[plan::PlanBatch],
    stale: Option<bool>,
) {
    let doc = serde_json::json!({
        "spec": spec_rel,
        "feature": feature,
        "scenarios": entries.iter().map(|e| {
            let status = record.and_then(|r| r.scenario_status(&e.scenario));
            serde_json::json!({
                "feature": e.feature,
                "scenario": e.scenario,
                "tags": e.tags,
                "line": e.line,
                "outline_rows": e.outline_rows,
                "status": status.map_or_else(|| serde_json::json!("unknown"), |s| serde_json::json!(s)),
            })
        }).collect::<Vec<_>>(),
        "batches": batches.iter().map(|b| {
            let (green, red, unknown) = batch_rollup(b, record);
            serde_json::json!({
                "batch": b.number,
                "scenarios": b.scenarios.len(),
                "green": green,
                "red": red,
                "unknown": unknown,
            })
        }).collect::<Vec<_>>(),
        "last_verify": record.map(|r| serde_json::json!({
            "recorded_at": r.recorded_at,
            "head": r.head,
            "outcome": r.outcome,
            "stale": stale,
        })),
    });
    println!("{doc:#}");
}

fn print_spec_status_human(
    spec_rel: &str,
    feature: &str,
    entries: &[spec::ScenarioEntry],
    record: Option<&craftsman::verify::record::LastVerify>,
    batches: &[plan::PlanBatch],
) {
    println!("Feature: {feature} ({spec_rel})");
    let mut greens = 0;
    let mut reds = 0;
    for e in entries {
        let tags = if e.tags.is_empty() {
            String::new()
        } else {
            format!("  [@{}]", e.tags.join(" @"))
        };
        let rows = e
            .outline_rows
            .map(|n| format!("  ({n} example rows)"))
            .unwrap_or_default();
        let status = record.and_then(|r| r.scenario_status(&e.scenario));
        let mark = status_mark(status);
        match mark {
            "pass" => greens += 1,
            "FAIL" | "unde" | "ambi" => reds += 1,
            _ => {}
        }
        println!("  {mark:<7}  {}  (line {}){tags}{rows}", e.scenario, e.line);
    }
    for b in batches {
        let (green, red, unknown) = batch_rollup(b, record);
        println!(
            "  batch {:<3} {green} green, {red} red, {unknown} unknown ({} scenarios)",
            b.number,
            b.scenarios.len()
        );
    }
    match record {
        Some(r) => println!(
            "{} scenarios — {greens} green, {reds} red, {} unknown (last verify {})",
            entries.len(),
            entries.len() - greens - reds,
            r.recorded_at
        ),
        None => println!(
            "{} scenarios — status unknown (no run results yet; run `craftsman verify`)",
            entries.len()
        ),
    }
}

fn spec_lint(json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let spec_path = loaded.root.join(&loaded.config.project.spec);

    // A spec that does not parse cannot be verified: report it as a lint
    // error finding (exit 1), not an orchestrator error. An unreadable spec
    // stays an orchestrator error (exit 3).
    let findings = match spec::parse_spec(&spec_path) {
        Ok(feature) => spec::lint(&feature),
        Err(err @ (spec::SpecError::Read { .. } | spec::SpecError::Write { .. })) => {
            return Err(err.into());
        }
        Err(spec::SpecError::Parse { message, .. }) => vec![spec::Finding {
            severity: Severity::Error,
            rule: "parse-error",
            line: 0,
            message,
        }],
    };

    let errors = count_errors(&findings);
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
        print_findings(&findings);
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

pub(super) fn count_errors(findings: &[spec::Finding]) -> usize {
    findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count()
}

pub(super) fn print_findings(findings: &[spec::Finding]) {
    for f in findings {
        let sev = match f.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        println!("{sev}[{}] line {}: {}", f.rule, f.line, f.message);
    }
}

fn spec_gen(json: bool, a11y_stub: bool) -> anyhow::Result<i32> {
    use craftsman::codegen::{self, Outcome};

    let cwd = cwd()?;
    if a11y_stub {
        let files = codegen::a11y_stub(&cwd)?;
        for f in &files {
            eprintln!("{:>8}  {}", f.action, f.path.display());
        }
        if json {
            let doc = serde_json::json!({ "generated": true, "files": files });
            println!("{doc:#}");
        }
        return Ok(EXIT_PASS);
    }
    let (code, files): (i32, Vec<codegen::FileReport>) = match codegen::run(&cwd)? {
        Outcome::LintErrors { errors } => {
            eprintln!(
                "spec gen refused: the spec has {errors} lint error(s) — every one \
                 breaks code generation; fix them first (run `craftsman spec lint`)"
            );
            (EXIT_VERIFICATION_FAILURE, Vec::new())
        }
        Outcome::NoCodegenStacks { stacks } => {
            eprintln!(
                "spec gen: no code-gen stack in [project] stacks {stacks:?} — \
                 only \"swift\" and \"bash\" need generated glue (exit 4)"
            );
            (EXIT_EMPTY_SELECTION, Vec::new())
        }
        Outcome::Generated(files) => {
            for f in &files {
                eprintln!("{:>8}  {}", f.action, f.path.display());
            }
            (EXIT_PASS, files)
        }
    };
    if json {
        let doc = serde_json::json!({
            "generated": code == EXIT_PASS,
            "files": files,
        });
        println!("{doc:#}");
    }
    Ok(code)
}

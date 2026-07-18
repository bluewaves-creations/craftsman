//! `craftsman verify` — THE gate.

use clap::Args;

use craftsman::verify::{self, Outcome, Selection};

use super::{EXIT_EMPTY_SELECTION, EXIT_PASS, EXIT_VERIFICATION_FAILURE, cwd};

#[derive(Args)]
pub struct VerifyArgs {
    /// Run only the scenarios listed under `## Batch N` in the plan
    #[arg(long, conflicts_with = "scenario")]
    pub batch: Option<u32>,
    /// Run a single scenario by exact name
    #[arg(long)]
    pub scenario: Option<String>,
    /// Run only scenarios the diff against REF (default HEAD) can
    /// affect, per the coverage map written by full verify runs.
    /// Falls back to running everything — loudly — when the map is
    /// missing or git cannot diff.
    #[arg(
        long,
        value_name = "REF",
        num_args = 0..=1,
        default_missing_value = "HEAD",
        conflicts_with_all = ["batch", "scenario"]
    )]
    pub impact: Option<String>,
    /// Emit the normalized results as JSON on stdout
    #[arg(long)]
    pub json: bool,
}

pub fn verify_cmd(args: &VerifyArgs) -> anyhow::Result<i32> {
    let selection = match (args.batch, &args.scenario, &args.impact) {
        (_, _, Some(reference)) => Selection::Impact(reference.clone()),
        (Some(n), _, None) => Selection::Batch(n),
        (None, Some(name), None) => Selection::Scenario(name.clone()),
        (None, None, None) => Selection::All,
    };
    let report = verify::run(&cwd()?, &selection)?;

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

    if args.json {
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

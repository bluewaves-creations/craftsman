//! `craftsman plan` — the plan's command layer.

use clap::Subcommand;

use craftsman::plan;
use craftsman::spec;

use super::spec::{count_errors, print_findings};
use super::{EXIT_PASS, EXIT_VERIFICATION_FAILURE, load};

#[derive(Subcommand)]
pub enum PlanCommand {
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

/// Dispatch `craftsman plan <lint>`.
pub fn plan_run(command: &PlanCommand) -> anyhow::Result<i32> {
    match command {
        PlanCommand::Lint { json } => plan_lint(*json),
    }
}

fn plan_lint(json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let feature = spec::parse_spec(&loaded.root.join(&loaded.config.project.spec))?;
    let names: Vec<String> = spec::inventory(&feature)
        .into_iter()
        .map(|e| e.scenario)
        .collect();
    let plan_rel = loaded.config.project.plan;
    let batches = plan::parse_plan(&loaded.root.join(&plan_rel))?;
    let findings = plan::lint(&batches, &names);

    let errors = count_errors(&findings);
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
        print_findings(&findings);
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

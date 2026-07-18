//! Selection resolution: map the user's `--batch/--scenario/--impact`
//! choice onto a runner filter against the spec inventory — or a finished
//! report that never reaches a runner (empty spec, unmatched name, empty
//! batch, impact "nothing affected").

use std::collections::HashSet;
use std::path::Path;

use super::{Outcome, Report, Selection, VerifyError, impact};
use crate::config::Config;
use crate::plan;

/// What selection resolution produced: a runner filter (`None` = run
/// everything), or a finished report that never reaches a runner.
pub(super) enum Resolved {
    Filter(Option<Vec<String>>),
    Finished(Report),
}

/// Resolve the user's selection against the spec inventory and (for
/// `--impact`) the impact map. Early-exit reports: empty spec, unmatched
/// scenario name, empty batch (all `EmptySelection`), and the impact
/// "nothing affected" verdict (`Passed` — see the comment there).
pub(super) fn resolve_selection(
    selection: &Selection,
    config: &Config,
    root: &Path,
    names: &[String],
    warnings: &mut Vec<String>,
) -> Result<Resolved, VerifyError> {
    let known: HashSet<&str> = names.iter().map(String::as_str).collect();
    if names.is_empty() {
        return Ok(Resolved::Finished(Report::empty(vec![format!(
            "spec {} contains no scenarios",
            config.project.spec
        )])));
    }
    let filter = match selection {
        Selection::All => None,
        Selection::Scenario(name) => {
            if !known.contains(name.as_str()) {
                return Ok(Resolved::Finished(Report::empty(vec![format!(
                    "no scenario named {name:?} in {}",
                    config.project.spec
                )])));
            }
            Some(vec![name.clone()])
        }
        Selection::Impact(reference) => {
            match resolve_impact(root, reference, names, warnings) {
                ImpactSelection::RunEverything => None,
                ImpactSelection::Subset(subset) if subset.is_empty() => {
                    // A computed empty set is a verdict from real coverage
                    // data (glue-mapped and unmapped scenarios always stay
                    // in), not a user filter typo — exit 0 with a loud
                    // note, not the exit-4 empty-selection error.
                    warnings.push(format!(
                        "impact: the diff against {reference} touches no file covered \
                         by any scenario — nothing to run (use `craftsman verify` \
                         without --impact to force a full run)"
                    ));
                    let mut report = Report::empty(std::mem::take(warnings));
                    report.outcome = Outcome::Passed;
                    return Ok(Resolved::Finished(report));
                }
                ImpactSelection::Subset(subset) => Some(subset),
            }
        }
        Selection::Batch(n) => {
            let requested = plan::batch_scenarios(&root.join(&config.project.plan), *n)?;
            let (found, missing): (Vec<String>, Vec<String>) = requested
                .into_iter()
                .partition(|s| known.contains(s.as_str()));
            for name in &missing {
                warnings.push(format!(
                    "plan batch {n} lists scenario {name:?} which is not in {} — \
                     plan drift; run `craftsman plan lint`",
                    config.project.spec
                ));
            }
            if found.is_empty() {
                return Ok(Resolved::Finished(Report::empty(std::mem::take(warnings))));
            }
            Some(found)
        }
    };
    Ok(Resolved::Filter(filter))
}

/// What impact resolution decided.
enum ImpactSelection {
    RunEverything,
    Subset(Vec<String>),
}

/// Resolve `--impact REF` into a scenario selection, falling back to a full
/// run — loudly, via `warnings` — whenever the map or git cannot answer
/// (cold start is never silently narrower).
fn resolve_impact(
    root: &Path,
    reference: &str,
    names: &[String],
    warnings: &mut Vec<String>,
) -> ImpactSelection {
    let Some(map) = impact::load(root) else {
        warnings.push(format!(
            "impact: no impact map at {} — running everything (a full \
             `craftsman verify` writes it)",
            impact::MAP_REL_PATH
        ));
        return ImpactSelection::RunEverything;
    };
    let changed = match impact::changed_files(root, reference) {
        Ok(changed) => changed,
        Err(err) => {
            warnings.push(format!("impact: {err} — running everything"));
            return ImpactSelection::RunEverything;
        }
    };
    let subset = impact::resolve(&map, &changed, names);
    if subset.len() == names.len() {
        ImpactSelection::RunEverything
    } else {
        warnings.push(format!(
            "impact: {} of {} scenarios affected by {} changed file(s) against {reference}",
            subset.len(),
            names.len(),
            changed.len()
        ));
        ImpactSelection::Subset(subset)
    }
}

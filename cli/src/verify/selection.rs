//! Selection resolution: map the user's `--batch/--scenario/--impact`
//! choice onto a runner filter against the spec inventory — or a finished
//! report that never reaches a runner (empty spec, unmatched name, empty
//! batch, impact "nothing affected").

use std::collections::HashSet;
use std::path::Path;

use super::{Outcome, Report, Selection, VerifyError, impact};
use crate::config::Config;
use crate::plan;

/// The `@requires-network` gate: scenarios carrying the tag never enter a
/// runner selection unless the live environment is explicitly granted —
/// the same `CRAFTSMAN_LIVE=1` switch the spec harness honors. Without it
/// they stay visible-unknown; a name filter must never force-run them
/// (cucumber-rs replaces the harness's programmatic filter with the
/// `--name` regex, so the runner cannot be trusted to hold this gate on
/// filtered paths).
pub(super) struct NetworkGate {
    gated: HashSet<String>,
    live: bool,
}

pub(super) const NETWORK_TAG: &str = "requires-network";

/// The live switch, read once per run: `CRAFTSMAN_LIVE=1`.
pub(super) fn live_env() -> bool {
    std::env::var("CRAFTSMAN_LIVE").is_ok_and(|v| v == "1")
}

impl NetworkGate {
    pub(super) const fn new(gated: HashSet<String>, live: bool) -> Self {
        Self { gated, live }
    }

    /// Build the gate from the spec inventory and the process environment.
    pub(super) fn from_inventory(entries: &[crate::spec::ScenarioEntry]) -> Self {
        Self::new(
            entries
                .iter()
                .filter(|e| e.tags.iter().any(|t| t == NETWORK_TAG))
                .map(|e| e.scenario.clone())
                .collect(),
            live_env(),
        )
    }

    /// Is `name` withheld from selection under this gate?
    fn excludes(&self, name: &str) -> bool {
        !self.live && self.gated.contains(name)
    }

    /// Drop gated names from a computed selection, warning once with the
    /// full list. `context` names the selection kind (e.g. "impact").
    fn split(&self, subset: Vec<String>, context: &str, warnings: &mut Vec<String>) -> Vec<String> {
        let (excluded, kept): (Vec<String>, Vec<String>) =
            subset.into_iter().partition(|n| self.excludes(n));
        if !excluded.is_empty() {
            warnings.push(format!(
                "{context}: excluded {} @{NETWORK_TAG} scenario(s) — set \
                 CRAFTSMAN_LIVE=1 to run: {}",
                excluded.len(),
                excluded.join(", ")
            ));
        }
        kept
    }
}

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
    gate: &NetworkGate,
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
            if gate.excludes(name) {
                return Ok(Resolved::Finished(Report::empty(vec![format!(
                    "scenario {name:?} is tagged @{NETWORK_TAG} — set \
                     CRAFTSMAN_LIVE=1 to run it live"
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
                ImpactSelection::Subset(subset) => {
                    let subset = gate.split(subset, "impact", warnings);
                    if subset.is_empty() {
                        // Everything the diff affects is network-gated:
                        // offline there is honestly nothing to run — the
                        // gated scenarios stay visible-unknown, and a
                        // commit gate must not go red over them.
                        let mut report = Report::empty(std::mem::take(warnings));
                        report.outcome = Outcome::Passed;
                        return Ok(Resolved::Finished(report));
                    }
                    Some(subset)
                }
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
            let found = gate.split(found, &format!("plan batch {n}"), warnings);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        toml::from_str("[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n")
            .expect("minimal config")
    }

    fn gate(gated: &[&str], live: bool) -> NetworkGate {
        NetworkGate::new(gated.iter().map(|s| (*s).to_owned()).collect(), live)
    }

    // Root-cause test (red before the fix): an explicitly selected
    // @requires-network scenario without CRAFTSMAN_LIVE must never reach
    // a runner — cucumber-rs would force-run it past the harness filter.
    #[test]
    fn gated_scenario_selection_without_live_never_reaches_a_runner() {
        let names = vec!["Update self-updates to the latest release".to_owned()];
        let mut warnings = Vec::new();
        let resolved = resolve_selection(
            &Selection::Scenario(names[0].clone()),
            &config(),
            Path::new("."),
            &names,
            &gate(&["Update self-updates to the latest release"], false),
            &mut warnings,
        )
        .expect("resolution succeeds");
        match resolved {
            Resolved::Finished(report) => {
                assert_eq!(report.outcome, Outcome::EmptySelection);
                assert!(
                    report.warnings.iter().any(|w| w.contains("CRAFTSMAN_LIVE")),
                    "the refusal must name the live switch: {:?}",
                    report.warnings
                );
            }
            Resolved::Filter(f) => panic!("must not reach a runner, got filter {f:?}"),
        }
    }

    #[test]
    fn gated_scenario_selection_with_live_reaches_the_runner() {
        let names = vec!["Update self-updates to the latest release".to_owned()];
        let mut warnings = Vec::new();
        let resolved = resolve_selection(
            &Selection::Scenario(names[0].clone()),
            &config(),
            Path::new("."),
            &names,
            &gate(&["Update self-updates to the latest release"], true),
            &mut warnings,
        )
        .expect("resolution succeeds");
        match resolved {
            Resolved::Filter(Some(filter)) => assert_eq!(filter, names),
            Resolved::Filter(None) => panic!("live selection must name the scenario"),
            Resolved::Finished(report) => {
                panic!(
                    "live selection must reach the runner: {:?}",
                    report.warnings
                );
            }
        }
    }

    // Root-cause test (red before the fix): a computed selection (impact,
    // batch) must drop gated scenarios with one loud warning, not run them.
    #[test]
    fn split_drops_gated_names_with_one_loud_warning() {
        let g = gate(&["Live only"], false);
        let mut warnings = Vec::new();
        let kept = g.split(
            vec!["Hermetic one".to_owned(), "Live only".to_owned()],
            "impact",
            &mut warnings,
        );
        assert_eq!(kept, vec!["Hermetic one".to_owned()]);
        assert_eq!(warnings.len(), 1, "exactly one warning: {warnings:?}");
        assert!(warnings[0].contains("Live only") && warnings[0].contains("CRAFTSMAN_LIVE"));
    }

    #[test]
    fn split_keeps_everything_when_live() {
        let g = gate(&["Live only"], true);
        let mut warnings = Vec::new();
        let kept = g.split(vec!["Live only".to_owned()], "impact", &mut warnings);
        assert_eq!(kept, vec!["Live only".to_owned()]);
        assert!(warnings.is_empty());
    }
}

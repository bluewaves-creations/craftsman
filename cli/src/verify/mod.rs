//! `craftsman verify` — THE gate: run SPEC.md scenarios via the stack
//! adapters and normalize every runner's output into the schema v1 results.
//!
//! Every stack listed in `[project] stacks` runs (rust, python, typescript,
//! and — Batch 5 — the code-gen stacks swift and bash); results merge into
//! one report with per-stack sections. Unknown stack names are rejected
//! upfront, before any adapter runs.
//!
//! Exit-code contract (enforced by the command layer from [`Outcome`]):
//! 0 all passed · 1 any failed/undefined/ambiguous · 3 tool or config
//! error · 4 empty selection (a filter or batch matching nothing is never
//! silent success — runners exit 0 on empty matches, so craftsman counts
//! scenarios itself; ADR-002/ADR-003).

pub mod adapters;
pub mod impact;
pub mod normalize;

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::Serialize;
use thiserror::Error;

use crate::config::{Config, ConfigError, VerifyStack};
use crate::plan::{self, PlanError};
use crate::spec::{self, SpecError};
use adapters::AdapterError;
use adapters::{bats, cucumber_js, cucumber_rs, pytest_bdd, swift_testing};
use normalize::{ScenarioResult, Status};

/// What to run: everything, one plan batch, one scenario by exact name, or
/// the scenarios a diff against a git ref can affect (the impact map).
#[derive(Debug, Clone)]
pub enum Selection {
    All,
    Batch(u32),
    Scenario(String),
    Impact(String),
}

/// Errors before or while running the adapters. Exit code 3 territory.
#[derive(Debug, Error)]
pub enum VerifyError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Spec(#[from] SpecError),
    #[error(transparent)]
    Plan(#[from] PlanError),
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error(
        "unknown stack {stack:?} in [project] stacks — craftsman verify \
         supports \"rust\", \"python\", \"typescript\", \"swift\", and \"bash\""
    )]
    UnknownStack { stack: String },
    #[error(transparent)]
    Gen(#[from] crate::codegen::GenError),
    #[error("[verify.{stack}] runner {runner:?} is not supported — use {supported:?}")]
    UnsupportedRunner {
        stack: &'static str,
        runner: String,
        supported: &'static str,
    },
}

/// Scenario counts by status.
#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct Counts {
    pub passed: usize,
    pub skipped: usize,
    pub pending: usize,
    pub undefined: usize,
    pub ambiguous: usize,
    pub failed: usize,
}

impl Counts {
    fn tally<'a>(results: impl IntoIterator<Item = &'a ScenarioResult>) -> Self {
        let mut c = Self::default();
        for r in results {
            match r.status {
                Status::Passed => c.passed += 1,
                Status::Skipped => c.skipped += 1,
                Status::Pending => c.pending += 1,
                Status::Undefined => c.undefined += 1,
                Status::Ambiguous => c.ambiguous += 1,
                Status::Failed => c.failed += 1,
            }
        }
        c
    }
}

/// The verify verdict, mapped 1:1 onto exit codes by the command layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Passed,
    Failed,
    EmptySelection,
}

/// One stack's normalized results — a section of the merged report.
#[derive(Debug, Serialize)]
pub struct StackReport {
    pub stack: String,
    pub results: Vec<ScenarioResult>,
}

/// Everything a verify run produced, merged across stacks.
#[derive(Debug)]
pub struct Report {
    pub stacks: Vec<StackReport>,
    pub counts: Counts,
    pub outcome: Outcome,
    /// Non-fatal drift, e.g. a plan batch naming scenarios absent from the
    /// spec (plan lint owns making that an error).
    pub warnings: Vec<String>,
}

impl Report {
    fn empty(warnings: Vec<String>) -> Self {
        Self {
            stacks: Vec::new(),
            counts: Counts::default(),
            outcome: Outcome::EmptySelection,
            warnings,
        }
    }

    /// All results across stacks, in stack order.
    pub fn results(&self) -> impl Iterator<Item = &ScenarioResult> {
        self.stacks.iter().flat_map(|s| s.results.iter())
    }
}

/// Run the verify gate for the project containing `cwd`.
///
/// # Errors
/// [`VerifyError`] on config/spec/plan/adapter failures — all exit code 3;
/// red scenarios are not an error but a [`Outcome::Failed`] report.
pub fn run(cwd: &Path, selection: &Selection) -> Result<Report, VerifyError> {
    let loaded = Config::load(cwd)?;
    let config = loaded.config;
    let root = loaded.root;

    // Reject unknown stacks before running anything: a config typo must
    // never silently shrink the verified surface.
    for stack in &config.project.stacks {
        if !matches!(
            stack.as_str(),
            "rust" | "python" | "typescript" | "swift" | "bash"
        ) {
            return Err(VerifyError::UnknownStack {
                stack: stack.clone(),
            });
        }
    }

    let feature = spec::parse_spec(&root.join(&config.project.spec))?;
    let names: Vec<String> = spec::inventory(&feature)
        .into_iter()
        .map(|e| e.scenario)
        .collect();

    // Resolve the selection against the spec inventory first: a filter that
    // cannot match anything is exit 4 without ever invoking a runner.
    let mut warnings = Vec::new();
    let filter = match resolve_selection(selection, &config, &root, &names, &mut warnings)? {
        Resolved::Finished(report) => return Ok(report),
        Resolved::Filter(filter) => filter,
    };

    // A full run (no filter) refreshes the impact map from what each stack
    // cheaply knows (see the impact module docs).
    let full_run = filter.is_none();
    let mut stacks = Vec::new();
    let mut stack_maps: BTreeMap<String, impact::StackMap> = BTreeMap::new();
    for stack in &config.project.stacks {
        let section = config.verify.stack(stack);
        let run = run_stack(
            stack,
            &root,
            section,
            &feature,
            &config.project.spec,
            filter.as_deref(),
            full_run,
        )?;
        if let Some(map) = run.map {
            stack_maps.insert(stack.clone(), map);
        }
        stacks.push(StackReport {
            stack: stack.clone(),
            results: run.results,
        });
    }
    if full_run
        && !stack_maps.is_empty()
        && let Err(err) = impact::save(&root, &impact::ImpactMap::new(stack_maps))
    {
        warnings.push(format!(
            "could not write {} ({err}) — the next --impact run falls back to --all",
            impact::MAP_REL_PATH
        ));
    }

    let counts = Counts::tally(stacks.iter().flat_map(|s| s.results.iter()));
    let total: usize = stacks.iter().map(|s| s.results.len()).sum();
    let outcome = if total == 0 {
        // Every runner's own view of the spec matched nothing (ADR-002:
        // empty matches exit 0 — the count is the only honest signal).
        Outcome::EmptySelection
    } else if counts.failed + counts.undefined + counts.ambiguous > 0 {
        Outcome::Failed
    } else {
        Outcome::Passed
    };

    Ok(Report {
        stacks,
        counts,
        outcome,
        warnings,
    })
}

/// What selection resolution produced: a runner filter (`None` = run
/// everything), or a finished report that never reaches a runner.
enum Resolved {
    Filter(Option<Vec<String>>),
    Finished(Report),
}

/// Resolve the user's selection against the spec inventory and (for
/// `--impact`) the impact map. Early-exit reports: empty spec, unmatched
/// scenario name, empty batch (all `EmptySelection`), and the impact
/// "nothing affected" verdict (`Passed` — see the comment there).
fn resolve_selection(
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

/// One stack's adapter run: results plus its impact-map contribution.
struct StackRun {
    results: Vec<ScenarioResult>,
    map: Option<impact::StackMap>,
}

/// Dispatch one stack to its adapter. The parsed `feature` anchors the
/// swift selection recipes (the generated `@Suite` struct name derives
/// from its name; xcodebuild selectors need each scenario's generated
/// test signature); `spec` is the spec's root-relative path for the
/// glue impact maps.
fn run_stack(
    stack: &str,
    root: &Path,
    section: Option<&VerifyStack>,
    feature: &gherkin::Feature,
    spec: &str,
    filter: Option<&[String]>,
    full_run: bool,
) -> Result<StackRun, VerifyError> {
    let project_dir = section
        .and_then(|s| s.cwd.as_ref())
        .map_or_else(|| root.to_path_buf(), |c| root.join(c));
    let check_runner = |stack: &'static str, supported: &'static str| {
        let runner = section
            .and_then(|s| s.runner.as_deref())
            .unwrap_or(supported);
        if runner == supported {
            Ok(())
        } else {
            Err(VerifyError::UnsupportedRunner {
                stack,
                runner: runner.to_owned(),
                supported,
            })
        }
    };

    // The stack's `cwd` as a root-relative prefix for impact-map paths
    // (git-diff paths are root-relative).
    let cwd_prefix = section.and_then(|s| s.cwd.as_deref());
    let prefixed = |rel: &str| {
        cwd_prefix.map_or_else(
            || rel.to_owned(),
            |cwd| format!("{}/{rel}", cwd.trim_end_matches('/')),
        )
    };
    let scenario_names = |results: &[ScenarioResult]| -> Vec<String> {
        results.iter().map(|r| r.scenario.clone()).collect()
    };

    match stack {
        "rust" => {
            check_runner("rust", "cucumber-rs")?;
            let runner_target = section
                .and_then(|s| s.runner_target.as_deref())
                .unwrap_or("spec");
            let results = cucumber_rs::run(&project_dir, runner_target, filter)?;
            // No cheap per-test coverage for cucumber-rs: the harness
            // target + the spec are the glue; `cwd` is the tree.
            let harness = prefixed(&format!("tests/{runner_target}.rs"));
            let map = full_run.then(|| spec_glue_map(&results, vec![harness], spec, cwd_prefix));
            Ok(StackRun { results, map })
        }
        "python" => {
            check_runner("python", "pytest-bdd")?;
            let tests_dir = section
                .and_then(|s| s.tests_dir.as_deref())
                .unwrap_or("tests");
            let artifacts = root.join(".craftsman").join("cache").join("verify");
            let run = pytest_bdd::run(&project_dir, &artifacts, tests_dir, filter, full_run)?;
            let map =
                run.coverage_json.as_deref().and_then(|doc| {
                    match impact::coverage_stack_map(doc, &scenario_names(&run.results), cwd_prefix)
                    {
                        Ok(map) => Some(map),
                        Err(err) => {
                            eprintln!(
                                "impact: unreadable coverage export ({err}) — python skipped"
                            );
                            None
                        }
                    }
                });
            Ok(StackRun {
                results: run.results,
                map,
            })
        }
        "typescript" => {
            check_runner("typescript", "cucumber-js")?;
            let artifacts = root.join(".craftsman").join("cache").join("verify");
            let results = cucumber_js::run(&project_dir, &artifacts, filter)?;
            let map = full_run
                .then(|| ts_stack_map(root, &project_dir, &artifacts, cwd_prefix, &results));
            Ok(StackRun { results, map })
        }
        "swift" => {
            check_runner("swift", "swift-testing")?;
            run_swift_stack(root, section, feature, spec, filter, full_run)
        }
        "bash" => {
            check_runner("bash", "bats")?;
            let bats_dir = crate::codegen::bats_dir(root, section);
            let results = bats::run(&bats_dir, filter)?;
            // No coverage concept for bats: the bats dir files + the spec
            // are the glue; `cwd` is the tree.
            let map = full_run.then(|| {
                spec_glue_map(
                    &results,
                    impact::files_under(&bats_dir, root),
                    spec,
                    cwd_prefix,
                )
            });
            Ok(StackRun { results, map })
        }
        // Defensive: `run` validates stack names upfront, so this arm only
        // fires if the two lists ever drift.
        other => Err(VerifyError::UnknownStack {
            stack: other.to_owned(),
        }),
    }
}

/// A glue map over `files` + the spec, tree-scoped to the stack's cwd
/// (rust and bash share this shape).
fn spec_glue_map(
    results: &[ScenarioResult],
    mut files: Vec<String>,
    spec: &str,
    tree: Option<&str>,
) -> impact::StackMap {
    files.push(spec.to_owned());
    impact::glue_stack_map(
        &results
            .iter()
            .map(|r| r.scenario.clone())
            .collect::<Vec<_>>(),
        files,
        tree.map(str::to_owned),
    )
}

/// The typescript impact map: per-scenario feature + step-definition
/// files from the Messages NDJSON the runner just wrote; coarse
/// `features/` glue fallback when it is missing or unreadable.
fn ts_stack_map(
    root: &Path,
    project_dir: &Path,
    artifacts: &Path,
    cwd_prefix: Option<&str>,
    results: &[ScenarioResult],
) -> impact::StackMap {
    let tree = cwd_prefix.map(str::to_owned);
    std::fs::read_to_string(artifacts.join("ts-msgs.ndjson"))
        .ok()
        .and_then(|ndjson| impact::messages_stack_map(&ndjson, cwd_prefix, tree.clone()).ok())
        .unwrap_or_else(|| {
            impact::glue_stack_map(
                &results
                    .iter()
                    .map(|r| r.scenario.clone())
                    .collect::<Vec<_>>(),
                impact::files_under(&project_dir.join("features"), root),
                tree,
            )
        })
}

/// The swift stack: the swift-testing adapter over the generated package,
/// or — when `[verify.swift] scheme` is set — the xcodebuild adapter over
/// the scheme (`codegen::swift` owns package/tests-dir resolution, the
/// suite name, and the generated test signatures).
fn run_swift_stack(
    root: &Path,
    section: Option<&VerifyStack>,
    feature: &gherkin::Feature,
    spec: &str,
    filter: Option<&[String]>,
    full_run: bool,
) -> Result<StackRun, VerifyError> {
    let package_dir = crate::codegen::swift::package_dir(root, section);
    let tests_dir = crate::codegen::swift::resolve_tests_dir(root, section)?;
    // SwiftPM convention: the test target is named after its source
    // directory under Tests/ — the filter recipe's Target anchor.
    let test_target = tests_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let suite = crate::codegen::swift::suite_name(&feature.name);
    let artifacts = root.join(".craftsman").join("cache").join("verify");
    let results = if let Some(scheme) = section.and_then(|s| s.scheme.as_deref()) {
        let selectors = filter
            .map(|names| xcodebuild_selectors(feature, &test_target, &suite, names))
            .transpose()?;
        adapters::xcodebuild::run(
            &package_dir,
            &artifacts,
            scheme,
            section.and_then(|s| s.destination.as_deref()),
            selectors.as_deref(),
        )?
    } else {
        swift_testing::run(&package_dir, &artifacts, &test_target, &suite, filter)?
    };
    // No cheap per-test coverage for swift: the generated runner + step
    // files + the spec are the glue; the package dir is the tree.
    let map = full_run.then(|| {
        let mut files = impact::files_under(&tests_dir, root);
        files.push(spec.to_owned());
        let tree = package_dir
            .strip_prefix(root)
            .ok()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_string_lossy().into_owned());
        impact::glue_stack_map(
            &results
                .iter()
                .map(|r| r.scenario.clone())
                .collect::<Vec<_>>(),
            files,
            tree,
        )
    });
    Ok(StackRun { results, map })
}

/// The `-only-testing:` identifiers for a set of scenario names — the
/// probed `` Target/Suite/`name`(signature) `` shape, with each signature
/// derived from the scenario's Examples headers.
fn xcodebuild_selectors(
    feature: &gherkin::Feature,
    test_target: &str,
    suite: &str,
    names: &[String],
) -> Result<Vec<String>, VerifyError> {
    names
        .iter()
        .map(|name| {
            // Selection is resolved against the spec inventory before any
            // adapter runs, so every name exists; an absent signature can
            // only mean spec/inventory drift within this process.
            let signature =
                crate::codegen::swift::test_signature(feature, name)?.unwrap_or_default();
            Ok(adapters::xcodebuild::only_testing_selector(
                test_target,
                suite,
                name,
                &signature,
            ))
        })
        .collect()
}

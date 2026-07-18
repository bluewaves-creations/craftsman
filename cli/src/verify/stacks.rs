//! Per-stack adapter dispatch: run one stack's runner, normalize its
//! results, and build its impact-map contribution.

use std::path::Path;

use gherkin::Feature;

use super::adapters::{self, bats, cucumber_js, cucumber_rs, pytest_bdd, swift_testing};
use super::normalize::ScenarioResult;
use super::{VerifyError, impact};
use crate::config::VerifyStack;

/// One stack's adapter run: results plus its impact-map contribution.
pub(super) struct StackRun {
    pub(super) results: Vec<ScenarioResult>,
    pub(super) map: Option<impact::StackMap>,
}

/// Dispatch one stack to its adapter. The parsed `feature` anchors the
/// swift selection recipes (the generated `@Suite` struct name derives
/// from its name; xcodebuild selectors need each scenario's generated
/// test signature); `spec` is the spec's root-relative path for the
/// glue impact maps.
pub(super) fn run_stack(
    stack: &str,
    root: &Path,
    section: Option<&VerifyStack>,
    feature: &Feature,
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
    feature: &Feature,
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
    feature: &Feature,
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

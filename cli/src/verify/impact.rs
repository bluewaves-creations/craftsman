//! The impact map behind `verify --impact` (the TDAD mechanism).
//!
//! After every full verify run, each stack records what it cheaply knows
//! about scenario → file dependencies into
//! `.craftsman/cache/impact-map.json`:
//!
//! - **python** (`kind: coverage`): per-test file coverage from pytest-cov
//!   test contexts (`coverage json --show-contexts`) — real, per-scenario
//!   executed-file sets. The only kind precise enough to EXCLUDE scenarios.
//! - **rust** (`kind: glue`): the cucumber-rs harness target file only —
//!   there is no cheap per-test coverage, so the mapping is informational
//!   and can never exclude a scenario.
//! - **typescript** (`kind: glue`): the feature + step files under
//!   `features/` — same informational role as rust.
//!
//! Resolution (`verify --impact [REF]`, default `HEAD`): changed files =
//! `git diff --name-only REF` plus untracked files, intersected with the
//! map. A scenario runs unless EVERY mapping it has is coverage-kind and
//! none of its covered files changed; scenarios with no map entry always
//! run (unknown = affected). A missing or unreadable map, or a failing git
//! diff, falls back to running everything with a loud note — conservative
//! correctness over cleverness: false positives cost seconds, false
//! negatives cost regressions.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::adapters::pytest_bdd::python_test_id;

/// Map location relative to the project root (gitignored cache).
pub const MAP_REL_PATH: &str = ".craftsman/cache/impact-map.json";

const MAP_VERSION: u32 = 1;

/// What a stack's mapping is allowed to claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MapKind {
    /// Real per-scenario coverage — may exclude unaffected scenarios.
    Coverage,
    /// Glue/harness files only — informational, never excludes.
    Glue,
}

/// One stack's scenario → root-relative file paths mapping.
#[derive(Debug, Serialize, Deserialize)]
pub struct StackMap {
    pub kind: MapKind,
    pub scenarios: BTreeMap<String, BTreeSet<String>>,
}

/// The whole persisted map.
#[derive(Debug, Serialize, Deserialize)]
pub struct ImpactMap {
    version: u32,
    pub stacks: BTreeMap<String, StackMap>,
}

impl ImpactMap {
    #[must_use]
    pub const fn new(stacks: BTreeMap<String, StackMap>) -> Self {
        Self {
            version: MAP_VERSION,
            stacks,
        }
    }
}

/// Errors collecting the git-side inputs of impact resolution. The caller
/// treats every one of them as "fall back to --all, loudly" — never fatal.
#[derive(Debug, Error)]
pub enum ImpactError {
    #[error("failed to spawn `git {args}` in {dir}")]
    GitSpawn {
        args: String,
        dir: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("`git {args}` failed: {detail}")]
    GitFailed { args: String, detail: String },
}

/// Load the map, or `None` when it is missing, unreadable, or from another
/// schema version — all equivalent to a cold start.
#[must_use]
pub fn load(root: &Path) -> Option<ImpactMap> {
    let text = std::fs::read_to_string(root.join(MAP_REL_PATH)).ok()?;
    let map: ImpactMap = serde_json::from_str(&text).ok()?;
    (map.version == MAP_VERSION).then_some(map)
}

/// Persist the map (single-writer: only the CLI touches `.craftsman/`).
///
/// # Errors
/// The underlying filesystem error; callers downgrade it to a warning —
/// the map is an optimization, never a verdict.
pub fn save(root: &Path, map: &ImpactMap) -> std::io::Result<()> {
    let path = root.join(MAP_REL_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string_pretty(map).map_err(std::io::Error::other)?;
    text.push('\n');
    std::fs::write(path, text)
}

/// The scenario subset the diff can affect, in `all` order (see module docs
/// for the inclusion rules).
#[must_use]
pub fn resolve(map: &ImpactMap, changed: &[String], all: &[String]) -> Vec<String> {
    let changed: HashSet<&str> = changed.iter().map(String::as_str).collect();
    all.iter()
        .filter(|name| {
            let mut mapped = false;
            let mut included = false;
            for stack in map.stacks.values() {
                if let Some(files) = stack.scenarios.get(name.as_str()) {
                    mapped = true;
                    included |= match stack.kind {
                        MapKind::Glue => true,
                        MapKind::Coverage => files.iter().any(|f| changed.contains(f.as_str())),
                    };
                }
            }
            !mapped || included
        })
        .cloned()
        .collect()
}

/// Root-relative files changed against `reference`, plus untracked files
/// (a brand-new step or source file is a change too).
///
/// # Errors
/// [`ImpactError`] when git cannot be spawned or exits non-zero (e.g. an
/// unborn HEAD) — callers fall back to running everything.
pub fn changed_files(root: &Path, reference: &str) -> Result<Vec<String>, ImpactError> {
    let mut files = git_lines(root, &["diff", "--name-only", reference])?;
    files.extend(git_lines(
        root,
        &["ls-files", "--others", "--exclude-standard"],
    )?);
    files.sort_unstable();
    files.dedup();
    Ok(files)
}

fn git_lines(root: &Path, args: &[&str]) -> Result<Vec<String>, ImpactError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|source| ImpactError::GitSpawn {
            args: args.join(" "),
            dir: root.to_path_buf(),
            source,
        })?;
    if !output.status.success() {
        return Err(ImpactError::GitFailed {
            args: args.join(" "),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// Build the python coverage-kind map from a `coverage json --show-contexts`
/// document.
///
/// Coverage file paths are relative to the stack's project dir;
/// `cwd_prefix` (the stack's `[verify.python] cwd`) rebases them onto the
/// repo root so they intersect with `git diff` paths.
///
/// # Errors
/// The serde error for an unparseable document; the caller downgrades it
/// to a warning.
pub fn coverage_stack_map(
    coverage_json: &str,
    scenarios: &[String],
    cwd_prefix: Option<&str>,
) -> Result<StackMap, serde_json::Error> {
    let doc: serde_json::Value = serde_json::from_str(coverage_json)?;

    // pytest-cov context: "<nodeid>|<phase>", nodeid "path::test_fn".
    // Collect test-fn → files that ran any line under it.
    let mut by_test: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    if let Some(files) = doc.get("files").and_then(serde_json::Value::as_object) {
        for (file, data) in files {
            let rebased = cwd_prefix.map_or_else(
                || file.clone(),
                |cwd| format!("{}/{file}", cwd.trim_end_matches('/')),
            );
            let contexts = data
                .get("contexts")
                .and_then(serde_json::Value::as_object)
                .into_iter()
                .flat_map(serde_json::Map::values)
                .filter_map(serde_json::Value::as_array)
                .flatten()
                .filter_map(serde_json::Value::as_str);
            for context in contexts {
                let nodeid = context.split('|').next().unwrap_or(context);
                let Some(test_fn) = nodeid.rsplit("::").next().filter(|f| !f.is_empty()) else {
                    continue;
                };
                // Strip any parametrization suffix ("test_x[row 1]").
                let test_fn = test_fn.split('[').next().unwrap_or(test_fn);
                by_test
                    .entry(test_fn.to_owned())
                    .or_default()
                    .insert(rebased.clone());
            }
        }
    }

    let scenarios_map: BTreeMap<String, BTreeSet<String>> = scenarios
        .iter()
        .filter_map(|name| {
            by_test
                .get(&python_test_id(name))
                .map(|files| (name.clone(), files.clone()))
        })
        .collect();
    Ok(StackMap {
        kind: MapKind::Coverage,
        scenarios: scenarios_map,
    })
}

/// A glue-kind map: every scenario points at the same harness/glue files.
#[must_use]
pub fn glue_stack_map(scenarios: &[String], files: Vec<String>) -> StackMap {
    let files: BTreeSet<String> = files.into_iter().collect();
    StackMap {
        kind: MapKind::Glue,
        scenarios: scenarios
            .iter()
            .map(|name| (name.clone(), files.clone()))
            .collect(),
    }
}

/// All files under `dir`, as paths relative to `strip` — the typescript
/// glue set (feature + step files). Missing dir → empty.
#[must_use]
pub fn files_under(dir: &Path, strip: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if let Ok(rel) = path.strip_prefix(strip) {
                out.push(rel.to_string_lossy().into_owned());
            }
        }
    }
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(names: &[&str]) -> Vec<String> {
        names.iter().map(|&n| n.to_owned()).collect()
    }

    fn map(stacks: Vec<(&str, StackMap)>) -> ImpactMap {
        ImpactMap::new(
            stacks
                .into_iter()
                .map(|(name, sm)| (name.to_owned(), sm))
                .collect(),
        )
    }

    fn coverage(entries: Vec<(&str, Vec<&str>)>) -> StackMap {
        StackMap {
            kind: MapKind::Coverage,
            scenarios: entries
                .into_iter()
                .map(|(name, files)| {
                    (
                        name.to_owned(),
                        files.into_iter().map(str::to_owned).collect(),
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn coverage_mapped_scenario_runs_only_when_its_files_change() {
        let m = map(vec![(
            "python",
            coverage(vec![
                ("Covered and touched", vec!["src/a.py"]),
                ("Covered and untouched", vec!["src/b.py"]),
            ]),
        )]);
        let all = owned(&["Covered and touched", "Covered and untouched"]);
        assert_eq!(
            resolve(&m, &owned(&["src/a.py"]), &all),
            owned(&["Covered and touched"])
        );
        assert!(resolve(&m, &owned(&["docs/readme.md"]), &all).is_empty());
    }

    #[test]
    fn unmapped_scenarios_always_run() {
        let m = map(vec![(
            "python",
            coverage(vec![("Mapped", vec!["src/a.py"])]),
        )]);
        let all = owned(&["Mapped", "Never seen before"]);
        assert_eq!(
            resolve(&m, &owned(&["unrelated.txt"]), &all),
            owned(&["Never seen before"])
        );
    }

    #[test]
    fn glue_mapped_scenarios_are_never_excluded() {
        let m = map(vec![(
            "rust",
            glue_stack_map(&owned(&["Glued"]), vec!["cli/tests/spec.rs".to_owned()]),
        )]);
        let all = owned(&["Glued"]);
        // Even a diff far away from the glue file keeps the scenario in:
        // glue maps are informational, not exclusion-grade.
        assert_eq!(resolve(&m, &owned(&["docs/readme.md"]), &all), all);
    }

    #[test]
    fn a_glue_mapping_overrides_a_dry_coverage_mapping() {
        // Same scenario known to two stacks: coverage says unaffected, glue
        // says "cannot know" — conservative union keeps it in.
        let m = map(vec![
            ("python", coverage(vec![("Shared", vec!["src/a.py"])])),
            (
                "rust",
                glue_stack_map(&owned(&["Shared"]), vec!["tests/spec.rs".to_owned()]),
            ),
        ]);
        let all = owned(&["Shared"]);
        assert_eq!(resolve(&m, &owned(&["docs/readme.md"]), &all), all);
    }

    #[test]
    fn resolution_preserves_spec_order() {
        let m = map(vec![(
            "python",
            coverage(vec![("B", vec!["b.py"]), ("A", vec!["a.py"])]),
        )]);
        let all = owned(&["B", "A"]);
        assert_eq!(
            resolve(&m, &owned(&["a.py", "b.py"]), &all),
            owned(&["B", "A"])
        );
    }

    #[test]
    fn coverage_stack_map_joins_contexts_to_scenarios() {
        // Shape observed from `coverage json --show-contexts` (pytest-cov
        // context "<nodeid>|<phase>").
        let doc = r#"{
            "files": {
                "tests/test_todo.py": {
                    "contexts": {
                        "5": ["", "tests/test_todo.py::test_add_an_item_to_the_list|run"],
                        "9": ["tests/test_todo.py::test_other_thing|run"]
                    }
                },
                "src/todo.py": {
                    "contexts": {
                        "2": ["tests/test_todo.py::test_add_an_item_to_the_list|run"]
                    }
                }
            }
        }"#;
        let scenarios = owned(&["Add an item to the list", "Unrelated scenario"]);
        let sm = coverage_stack_map(doc, &scenarios, Some("backend")).expect("parses");
        assert_eq!(sm.kind, MapKind::Coverage);
        let files = &sm.scenarios["Add an item to the list"];
        assert!(files.contains("backend/tests/test_todo.py"));
        assert!(files.contains("backend/src/todo.py"));
        // No context matched the second scenario: no entry → always runs.
        assert!(!sm.scenarios.contains_key("Unrelated scenario"));
    }

    #[test]
    fn load_rejects_other_versions() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let m = map(vec![("python", coverage(vec![("S", vec!["a.py"])]))]);
        save(tmp.path(), &m).expect("save");
        let loaded = load(tmp.path()).expect("round trip");
        assert_eq!(loaded.stacks["python"].scenarios["S"].len(), 1);

        let path = tmp.path().join(MAP_REL_PATH);
        std::fs::write(&path, r#"{"version": 99, "stacks": {}}"#).expect("write");
        assert!(
            load(tmp.path()).is_none(),
            "future versions are cold starts"
        );
        std::fs::write(&path, "not json").expect("write");
        assert!(load(tmp.path()).is_none(), "garbage is a cold start");
    }
}

//! The impact map behind `verify --impact` (the TDAD mechanism).
//!
//! After every full verify run, each stack records what it cheaply knows
//! about scenario → file dependencies into
//! `.craftsman/cache/impact-map.json`:
//!
//! - **python** (`kind: coverage`): per-test file coverage from pytest-cov
//!   test contexts (`coverage json --show-contexts`) — real, per-scenario
//!   executed-file sets, excluding at scenario granularity.
//! - **typescript** (`kind: glue`): per-scenario feature file (pickle
//!   `uri`) + step-definition files (`stepDefinition` source references
//!   joined through the `testCase` steps) from the Messages NDJSON the
//!   runner already wrote (Batch 9a; falls back to the files under
//!   `features/`).
//! - **rust** (`kind: glue`): the cucumber-rs harness target file + the
//!   spec. **swift**: the generated runner + step files + the spec.
//!   **bash**: the bats dir files + the spec.
//!
//! Glue maps also carry the stack's `tree` — the root-relative directory
//! owning its code (`[verify.<stack>] cwd`/package dir; absent = the
//! whole repo). Resolution per scenario: a coverage mapping includes it
//! when one of its covered files changed; a glue mapping includes it when
//! any glue file changed (glue change = run everything) OR any changed
//! file lives under the stack's tree (product code is never mapped
//! per-scenario — conservative). A diff touching neither — docs-only, or
//! another stack's tree — genuinely narrows to zero for that stack
//! (Batch 9a; the dispatcher reports that loudly with exit 0). Scenarios
//! with no map entry always run (unknown = affected); a missing or
//! unreadable map, or a failing git diff, falls back to running
//! everything with a loud note.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::adapters::pytest_bdd::python_test_id;

/// Map location relative to the project root (gitignored cache).
pub const MAP_REL_PATH: &str = ".craftsman/cache/impact-map.json";

/// v2 (Batch 9a): glue maps gained `tree`; v1 maps are cold starts.
const MAP_VERSION: u32 = 2;

/// What a stack's mapping is allowed to claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MapKind {
    /// Real per-scenario coverage — excludes at scenario granularity.
    Coverage,
    /// Glue/harness files + the stack tree — excludes only when the diff
    /// touches neither (module docs).
    Glue,
}

/// One stack's scenario → root-relative file paths mapping.
#[derive(Debug, Serialize, Deserialize)]
pub struct StackMap {
    pub kind: MapKind,
    /// Root-relative directory owning the stack's code (glue kind only);
    /// `None` = the whole repo, so any change keeps the stack in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<String>,
    pub scenarios: BTreeMap<String, BTreeSet<String>>,
}

impl StackMap {
    /// Whether the diff touches this glue-kind stack: any glue file
    /// changed, or any changed file under its tree (`None` = anywhere).
    fn glue_hot(&self, changed: &[String], changed_set: &HashSet<&str>) -> bool {
        let glue_touched = self
            .scenarios
            .values()
            .flatten()
            .any(|f| changed_set.contains(f.as_str()));
        let tree_touched = self.tree.as_ref().map_or_else(
            || !changed.is_empty(),
            |tree| {
                let prefix = format!("{}/", tree.trim_end_matches('/'));
                changed.iter().any(|c| c.starts_with(&prefix))
            },
        );
        glue_touched || tree_touched
    }
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
    let changed_set: HashSet<&str> = changed.iter().map(String::as_str).collect();
    // Glue heat is a per-stack fact (any glue file or in-tree change =
    // that stack runs everything it maps).
    let stacks: Vec<(&StackMap, bool)> = map
        .stacks
        .values()
        .map(|s| {
            (
                s,
                s.kind == MapKind::Glue && s.glue_hot(changed, &changed_set),
            )
        })
        .collect();
    all.iter()
        .filter(|name| {
            let mut mapped = false;
            let mut included = false;
            for (stack, hot) in &stacks {
                if let Some(files) = stack.scenarios.get(name.as_str()) {
                    mapped = true;
                    included |= match stack.kind {
                        MapKind::Glue => *hot,
                        MapKind::Coverage => files.iter().any(|f| changed_set.contains(f.as_str())),
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
        tree: None,
        scenarios: scenarios_map,
    })
}

/// A glue-kind map: every scenario points at the same harness/glue files;
/// `tree` is the stack's root-relative code directory (`None` = the whole
/// repo — such a map can never exclude anything).
#[must_use]
pub fn glue_stack_map(scenarios: &[String], files: Vec<String>, tree: Option<String>) -> StackMap {
    let files: BTreeSet<String> = files.into_iter().collect();
    StackMap {
        kind: MapKind::Glue,
        tree,
        scenarios: scenarios
            .iter()
            .map(|name| (name.clone(), files.clone()))
            .collect(),
    }
}

/// The typescript per-scenario map from the Messages NDJSON.
///
/// The runner already wrote it: scenario → its feature file (pickle
/// `uri`) + the step-definition files its `testCase` steps resolved to
/// (`stepDefinition.sourceReference.uri`). Paths are runner-cwd-relative
/// in the messages; `cwd_prefix` rebases them onto the repo root.
///
/// # Errors
/// The serde error for an unparseable document; the caller downgrades it
/// to the coarse `features/` glue map.
pub fn messages_stack_map(
    ndjson: &str,
    cwd_prefix: Option<&str>,
    tree: Option<String>,
) -> Result<StackMap, serde_json::Error> {
    use serde_json::Value;

    let rebase = |uri: &str| {
        cwd_prefix.map_or_else(
            || uri.to_owned(),
            |cwd| format!("{}/{uri}", cwd.trim_end_matches('/')),
        )
    };
    // stepDefinitionId → uri · pickleId → (name, uri) · then join testCases.
    let mut stepdef_uris: BTreeMap<String, String> = BTreeMap::new();
    let mut pickles: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut scenarios: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in ndjson.lines().filter(|l| !l.trim().is_empty()) {
        let m: Value = serde_json::from_str(line)?;
        if let Some(def) = m.get("stepDefinition") {
            if let (Some(id), Some(uri)) = (
                def.get("id").and_then(Value::as_str),
                def.pointer("/sourceReference/uri").and_then(Value::as_str),
            ) {
                stepdef_uris.insert(id.to_owned(), rebase(uri));
            }
        } else if let Some(pickle) = m.get("pickle") {
            if let (Some(id), Some(name), Some(uri)) = (
                pickle.get("id").and_then(Value::as_str),
                pickle.get("name").and_then(Value::as_str),
                pickle.get("uri").and_then(Value::as_str),
            ) {
                pickles.insert(id.to_owned(), (name.to_owned(), rebase(uri)));
            }
        } else if let Some(test_case) = m.get("testCase") {
            join_test_case(test_case, &pickles, &stepdef_uris, &mut scenarios);
        }
    }
    Ok(StackMap {
        kind: MapKind::Glue,
        tree,
        scenarios,
    })
}

/// One `testCase` message: pickle name + uri, plus every step's
/// step-definition file.
fn join_test_case(
    test_case: &serde_json::Value,
    pickles: &BTreeMap<String, (String, String)>,
    stepdef_uris: &BTreeMap<String, String>,
    scenarios: &mut BTreeMap<String, BTreeSet<String>>,
) {
    use serde_json::Value;

    let Some((name, uri)) = test_case
        .get("pickleId")
        .and_then(Value::as_str)
        .and_then(|pid| pickles.get(pid))
    else {
        return;
    };
    let entry = scenarios.entry(name.clone()).or_default();
    entry.insert(uri.clone());
    let step_ids = test_case
        .get("testSteps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| s.get("stepDefinitionIds").and_then(Value::as_array))
        .flatten()
        .filter_map(Value::as_str);
    for id in step_ids {
        if let Some(step_uri) = stepdef_uris.get(id) {
            entry.insert(step_uri.clone());
        }
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
            tree: None,
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
    fn treeless_glue_mapped_scenarios_are_never_excluded() {
        let m = map(vec![(
            "rust",
            glue_stack_map(
                &owned(&["Glued"]),
                vec!["cli/tests/spec.rs".to_owned()],
                None,
            ),
        )]);
        let all = owned(&["Glued"]);
        // No tree recorded = the whole repo is the stack's code: even a
        // diff far away from the glue file keeps the scenario in.
        assert_eq!(resolve(&m, &owned(&["docs/readme.md"]), &all), all);
    }

    #[test]
    fn a_glue_mapping_overrides_a_dry_coverage_mapping() {
        // Same scenario known to two stacks: coverage says unaffected,
        // tree-less glue says "cannot know" — conservative union keeps it.
        let m = map(vec![
            ("python", coverage(vec![("Shared", vec!["src/a.py"])])),
            (
                "rust",
                glue_stack_map(&owned(&["Shared"]), vec!["tests/spec.rs".to_owned()], None),
            ),
        ]);
        let all = owned(&["Shared"]);
        assert_eq!(resolve(&m, &owned(&["docs/readme.md"]), &all), all);
    }

    #[test]
    fn tree_scoped_glue_narrows_docs_only_and_other_stack_diffs_to_zero() {
        // Batch 9a: the rust stack maps its harness + spec, tree "cli".
        let m = map(vec![(
            "rust",
            glue_stack_map(
                &owned(&["A", "B"]),
                vec!["cli/tests/spec.rs".to_owned(), "SPEC.md".to_owned()],
                Some("cli".to_owned()),
            ),
        )]);
        let all = owned(&["A", "B"]);
        // Docs-only diff: outside the glue set and the tree → zero runs.
        assert!(resolve(&m, &owned(&["docs/readme.md"]), &all).is_empty());
        // Another stack's tree: same verdict.
        assert!(resolve(&m, &owned(&["web/src/app.ts"]), &all).is_empty());
        // Any in-tree change (product code is never mapped per-scenario)
        // runs everything the stack maps.
        assert_eq!(resolve(&m, &owned(&["cli/src/lib.rs"]), &all), all);
        // A glue change (the spec lives outside the tree) also runs all.
        assert_eq!(resolve(&m, &owned(&["SPEC.md"]), &all), all);
        // A prefix-sharing sibling dir is NOT the tree.
        assert!(resolve(&m, &owned(&["cli-docs/x.md"]), &all).is_empty());
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
    fn messages_stack_map_joins_pickles_and_step_definitions() {
        // Shapes per the Cucumber Messages NDJSON fixture (ADR-002).
        let ndjson = concat!(
            r#"{"stepDefinition":{"id":"sd1","sourceReference":{"uri":"features/support/steps.ts","location":{"line":6}}}}"#,
            "\n",
            r#"{"pickle":{"id":"p1","uri":"features/todo.feature","name":"Add an item"}}"#,
            "\n",
            r#"{"testCase":{"id":"tc1","pickleId":"p1","testSteps":[{"id":"s1","stepDefinitionIds":["sd1"]}]}}"#,
            "\n",
        );
        let sm = messages_stack_map(ndjson, Some("web"), Some("web".to_owned())).expect("parses");
        assert_eq!(sm.kind, MapKind::Glue);
        assert_eq!(sm.tree.as_deref(), Some("web"));
        let files = &sm.scenarios["Add an item"];
        assert!(files.contains("web/features/todo.feature"), "{files:?}");
        assert!(files.contains("web/features/support/steps.ts"), "{files:?}");
    }

    #[test]
    fn messages_stack_map_reads_the_real_cucumber_js_fixture() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/msgs.ndjson"
        ))
        .expect("fixture");
        let sm = messages_stack_map(&text, None, None).expect("parses");
        let files = &sm.scenarios["Add an item to the list"];
        assert!(files.contains("features/todo.feature"), "{files:?}");
        assert!(
            files.contains("features/step_definitions/steps.mjs"),
            "{files:?}"
        );
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

//! Stack-map builders: turn what each runner already produced (a coverage
//! export, the Messages NDJSON, or plain glue-file lists) into a
//! [`StackMap`] contribution for the impact map.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::super::adapters::pytest_bdd::python_test_id;
use super::{MapKind, StackMap};

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
}

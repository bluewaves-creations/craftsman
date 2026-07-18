//! Step definitions — the recovered verify/impact scenarios (Batch 11):
//! the positive scenario filter, undefined-step evidence, the coverage
//! impact map, impact narrowing, and the unmapped-always-runs rule. All
//! run on real pytest-bdd fixtures (uv-driven, cached at stable paths).

use std::path::PathBuf;

use cucumber::{given, then, when};

use crate::CliWorld;

fn python_todo_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/python-todo")
}

/// A minimal two-module pytest-bdd project: scenario "First behavior"
/// executes `src/a.py`, "Second behavior" executes `src/b.py` (imports
/// happen inside the steps so coverage attributes each module to its own
/// scenario). Reuses python-todo's pyproject + uv.lock for a warm cache.
fn build_two_module_fixture(w: &mut CliWorld, dir_name: &str, feature: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(dir_name);
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["features", "tests", "src"] {
        std::fs::create_dir_all(dir.join(sub)).expect("mkdirs");
    }
    let todo = python_todo_fixture();
    for file in ["pyproject.toml", "uv.lock"] {
        std::fs::copy(todo.join(file), dir.join(file))
            .unwrap_or_else(|e| panic!("copy {file}: {e}"));
    }
    let write = |rel: &str, content: &str| {
        std::fs::write(dir.join(rel), content).unwrap_or_else(|e| panic!("write {rel}: {e}"));
    };
    write(
        "craftsman.toml",
        "[project]\nname = \"impact-fixture\"\nstacks = [\"python\"]\nspec = \"features/fixture.feature\"\n\n[verify.python]\nrunner = \"pytest-bdd\"\ntests-dir = \"tests\"\n",
    );
    write("features/fixture.feature", feature);
    write("conftest.py", "");
    write("src/__init__.py", "");
    write("src/a.py", "def truth():\n    return True\n");
    write("src/b.py", "def truth():\n    return True\n");
    write(
        "tests/test_fixture.py",
        "from pytest_bdd import given, scenarios\n\nscenarios(\"../features/fixture.feature\")\n\n\n@given(\"the a module truth holds\")\ndef a_truth():\n    from src.a import truth\n\n    assert truth()\n\n\n@given(\"the b module truth holds\")\ndef b_truth():\n    from src.b import truth\n\n    assert truth()\n",
    );
    write(".gitignore", ".craftsman/\n.venv/\n__pycache__/\n");
    w.fixed_dir = Some(dir.clone());
    dir
}

const TWO_SCENARIO_FEATURE: &str = "Feature: Impact fixture\n\n  Scenario: First behavior\n    Given the a module truth holds\n\n  Scenario: Second behavior\n    Given the b module truth holds\n";

const ONE_SCENARIO_FEATURE: &str =
    "Feature: Impact fixture\n\n  Scenario: First behavior\n    Given the a module truth holds\n";

fn prime_full_verify(w: &mut CliWorld) {
    w.run_craftsman(&["verify"]);
    assert_eq!(
        w.output().status.code(),
        Some(0),
        "priming verify must pass:\n{}",
        w.combined_output()
    );
}

#[given(expr = "a craftsman project with passing scenarios {string} and {string}")]
fn project_with_passing_scenarios(w: &mut CliWorld, first: String, second: String) {
    assert_eq!(
        (first.as_str(), second.as_str()),
        ("First behavior", "Second behavior"),
        "the fixture feature encodes exactly these scenario names"
    );
    build_two_module_fixture(w, "craftsman-spec-pyfilter-fixture", TWO_SCENARIO_FEATURE);
}

/// The per-scenario result lines of the verify output (`pass`/`FAIL`).
fn result_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            (t.starts_with("pass ") || t.starts_with("FAIL ")) && l.starts_with("  ")
        })
        .map(str::to_owned)
        .collect()
}

#[then(expr = "exactly one scenario result is reported, named {string}")]
fn exactly_one_result(w: &mut CliWorld, name: String) {
    let combined = w.combined_output();
    let results = result_lines(&combined);
    assert_eq!(results.len(), 1, "expected one result line:\n{combined}");
    assert!(
        results[0].contains(&name),
        "the one result must be {name:?}:\n{combined}"
    );
}

#[given("a craftsman project whose spec has a scenario with an unimplemented step")]
fn project_with_unimplemented_step(w: &mut CliWorld) {
    let dir = std::env::temp_dir().join("craftsman-spec-pyundef-fixture");
    let _ = std::fs::remove_dir_all(dir.join(".craftsman"));
    crate::repo_steps::copy_tree(&python_todo_fixture(), &dir);
    w.fixed_dir = Some(dir);
}

#[then("the undefined scenario result carries the runner's missing-step detail")]
fn undefined_carries_detail(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("StepDefinitionNotFoundError"),
        "the runner's missing-step evidence must be printed:\n{combined}"
    );
}

#[given("a python craftsman project with no impact map")]
fn python_project_without_map(w: &mut CliWorld) {
    let dir = std::env::temp_dir().join("craftsman-spec-pymap-fixture");
    crate::repo_steps::copy_tree(&python_todo_fixture(), &dir);
    let _ = std::fs::remove_dir_all(dir.join(".craftsman"));
    w.fixed_dir = Some(dir);
}

#[then("an impact map exists mapping each covered scenario to the files it executed")]
fn impact_map_exists(w: &mut CliWorld) {
    let path = w.project_dir().join(".craftsman/cache/impact-map.json");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc: serde_json::Value = serde_json::from_str(&text).expect("map is valid JSON");
    assert_eq!(doc["stacks"]["python"]["kind"], "coverage", "{doc:#}");
    let files = doc["stacks"]["python"]["scenarios"]["Add an item to the list"]
        .as_array()
        .unwrap_or_else(|| panic!("no covered files for the passing scenario: {doc:#}"));
    assert!(
        files.iter().any(|f| f == "tests/test_todo.py"),
        "coverage must name the executed test module: {files:?}"
    );
}

fn map_covers(w: &mut CliWorld, scenario: &str, file: &str) {
    let path = w.project_dir().join(".craftsman/cache/impact-map.json");
    let text = std::fs::read_to_string(&path).expect("impact map");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("map JSON");
    let files = doc["stacks"]["python"]["scenarios"][scenario]
        .as_array()
        .unwrap_or_else(|| panic!("no map entry for {scenario}: {doc:#}"));
    assert!(
        files.iter().any(|f| f == file),
        "{scenario} must cover {file}: {files:?}"
    );
}

#[given(
    expr = "a craftsman project whose impact map covers {string} with src\\/a.py and {string} with src\\/b.py"
)]
fn project_with_covering_map(w: &mut CliWorld, first: String, second: String) {
    let dir = build_two_module_fixture(w, "craftsman-spec-pyimpact-fixture", TWO_SCENARIO_FEATURE);
    prime_full_verify(w);
    map_covers(w, &first, "src/a.py");
    map_covers(w, &second, "src/b.py");
    crate::repo_steps::git_init_commit_all(&dir);
}

#[given(expr = "the diff since the last commit touches only src\\/a.py")]
fn diff_touches_only_a(w: &mut CliWorld) {
    let path = w.project_dir().join("src/a.py");
    let mut text = std::fs::read_to_string(&path).expect("read src/a.py");
    text.push_str("\n# touched by the impact scenario\n");
    std::fs::write(&path, text).expect("write src/a.py");
}

#[when("I run craftsman verify with impact selection")]
fn run_verify_impact(w: &mut CliWorld) {
    w.run_craftsman(&["verify", "--impact"]);
}

#[then(expr = "only the scenario {string} runs")]
fn only_named_scenario_runs(w: &mut CliWorld, name: String) {
    let combined = w.combined_output();
    let results = result_lines(&combined);
    assert_eq!(
        results.len(),
        1,
        "expected exactly one scenario to run:\n{combined}"
    );
    assert!(
        results[0].contains(&name),
        "the running scenario must be {name:?}:\n{combined}"
    );
}

#[given(expr = "a craftsman project whose impact map covers {string} but not {string}")]
fn project_with_partial_map(w: &mut CliWorld, first: String, unmapped: String) {
    let dir = build_two_module_fixture(w, "craftsman-spec-pynew-fixture", ONE_SCENARIO_FEATURE);
    prime_full_verify(w);
    map_covers(w, &first, "src/a.py");
    // The new scenario arrives after the map was recorded — the real
    // situation the rule exists for.
    std::fs::write(
        dir.join("features/fixture.feature"),
        format!(
            "{ONE_SCENARIO_FEATURE}\n  Scenario: {unmapped}\n    Given the a module truth holds\n"
        ),
    )
    .expect("extend feature");
    crate::repo_steps::git_init_commit_all(&dir);
}

#[given("the diff since the last commit touches no covered file")]
fn diff_touches_nothing_covered(w: &mut CliWorld) {
    w.write("README.md", "# uncovered file\n");
}

#[given("a scaffolded rust project with a recorded green verify run and a clean tree")]
fn scaffolded_recorded_clean_project(w: &mut CliWorld) {
    crate::project_steps::scaffold_green_fixture(w, "craftsman-spec-impact-empty-fixture");
    let dir = w.project_dir();
    let _ = std::fs::remove_dir_all(dir.join(".git"));
    std::fs::write(dir.join(".gitignore"), "target/\n.craftsman/\nCargo.lock\n")
        .expect("write .gitignore");
    crate::repo_steps::git_init_commit_all(&dir);
    prime_full_verify(w);
}

#[then(expr = "the scenario {string} runs")]
fn named_scenario_runs(w: &mut CliWorld, name: String) {
    let combined = w.combined_output();
    let results = result_lines(&combined);
    assert!(
        results.iter().any(|l| l.contains(&name)),
        "{name:?} must be among the run scenarios:\n{combined}"
    );
}

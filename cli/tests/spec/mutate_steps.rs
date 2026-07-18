//! Step definitions — the recovered mutate scenarios (Batch 11): the
//! survivor threshold block (real mutmut over the python mutation
//! fixture), the clean-tree pass, and the no-consensus-tool refusal.

use std::path::PathBuf;

use cucumber::given;

use crate::CliWorld;

fn python_todo_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/python-todo")
}

#[given("a craftsman project whose diff touches code with weak tests")]
fn project_with_weak_tests(w: &mut CliWorld) {
    let dir = w.project_dir();
    std::fs::create_dir_all(dir.join("tests")).expect("mkdirs");
    let todo = python_todo_fixture();
    for (src, dest) in [
        ("pyproject.toml", "pyproject.toml"),
        ("uv.lock", "uv.lock"),
        ("mutation/todo_util.py", "todo_util.py"),
        ("mutation/tests/test_util.py", "tests/test_util.py"),
    ] {
        std::fs::copy(todo.join(src), dir.join(dest)).unwrap_or_else(|e| panic!("copy {src}: {e}"));
    }
    w.write(
        "craftsman.toml",
        "[project]\nname = \"mutate-py\"\nstacks = [\"python\"]\n\n[verify.python]\ntests-dir = \"tests\"\n",
    );
    w.write(".gitignore", ".craftsman/\n.venv/\n__pycache__/\n");
    crate::repo_steps::git_init_commit_all(&dir);
    let path = dir.join("todo_util.py");
    let mut text = std::fs::read_to_string(&path).expect("read todo_util.py");
    text.push_str("\n# seeded diff for the mutate scenario\n");
    std::fs::write(&path, text).expect("write todo_util.py");
}

#[given("the mutate minimum score is 100")]
fn mutate_min_score_100(w: &mut CliWorld) {
    let path = w.project_dir().join("craftsman.toml");
    let mut text = std::fs::read_to_string(&path).expect("read config");
    text.push_str("\n[mutate]\nmin-score = 100\n");
    std::fs::write(&path, text).expect("write config");
}

#[cucumber::then("the output reports the score against the threshold")]
fn output_reports_score(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("score") && combined.contains("threshold 100"),
        "the score must be reported against its threshold:\n{combined}"
    );
}

#[cucumber::then("survived mutants are reported as findings")]
fn survivors_are_findings(w: &mut CliWorld) {
    let combined = w.combined_output();
    assert!(
        combined.contains("survived"),
        "survivors must be visible findings:\n{combined}"
    );
}

#[given("a craftsman project with no uncommitted changes")]
fn project_with_clean_tree(w: &mut CliWorld) {
    w.write(
        "craftsman.toml",
        "[project]\nname = \"fixture\"\nstacks = [\"python\"]\n\n[verify.python]\ntests-dir = \"tests\"\n",
    );
    w.write("module.py", "def truth():\n    return True\n");
    w.write(".gitignore", ".craftsman/\n");
    crate::repo_steps::git_init_commit_all(&w.project_dir());
}

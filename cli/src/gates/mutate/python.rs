//! The python mutate runner: mutmut over changed files, uv-driven;
//! verdicts from the spinner's final counts segment (2.5.1 reality).

use std::path::Path;

use super::super::{Finding, GateError, Severity, exec, tail};
use super::{
    Config, MUTMUT_VERSION, Scope, StackRun, Tally, changed_stack_files, pinned, scope_word,
    tracked_stack_files,
};

pub(super) fn python_mutate(
    root: &Path,
    config: &Config,
    cwd: Option<&str>,
    scope: Scope,
) -> Result<StackRun, GateError> {
    let version = pinned(config, "mutmut", MUTMUT_VERSION);
    let dir = cwd.map_or_else(|| root.to_path_buf(), |c| root.join(c));
    let paths = match scope {
        Scope::Diff => {
            let changed = changed_stack_files(root, cwd, &["py"])?;
            if changed.is_empty() {
                return Ok(StackRun::skipped(
                    "mutmut",
                    "mutate[python]: no changed .py files — nothing to mutate".to_owned(),
                ));
            }
            changed
        }
        Scope::All => {
            let all: Vec<String> = tracked_stack_files(root, cwd, &["py"])?;
            if all.is_empty() {
                return Ok(StackRun::skipped(
                    "mutmut",
                    "mutate[python]: no tracked .py files".to_owned(),
                ));
            }
            all
        }
    };
    let tests_dir = config
        .verify
        .stack("python")
        .and_then(|s| s.tests_dir.as_deref())
        .unwrap_or("tests")
        .to_owned();
    // File-granular scoping via mutmut 2.5.1's --paths-to-mutate; test
    // files themselves are excluded from mutation.
    let sources: Vec<String> = paths
        .iter()
        .filter(|p| !p.starts_with(&format!("{tests_dir}/")))
        .cloned()
        .collect();
    if sources.is_empty() {
        return Ok(StackRun::skipped(
            "mutmut",
            "mutate[python]: only test files changed — nothing to mutate".to_owned(),
        ));
    }
    let argv = vec![
        "uv".to_owned(),
        "run".to_owned(),
        // mutmut 2.5.1 crashes under Python 3.14 (deepcopy recursion;
        // observed live on CI — run 29645260578) but runs under 3.13.
        // Pin the newest surviving interpreter for this invocation; uv
        // provisions it. Must run in the PROJECT env (tests need project
        // deps), so a project requiring >= 3.14 makes mutmut genuinely
        // unusable — uv's version-conflict error then surfaces through the
        // loud no-verdict refusal, which is the correct outcome.
        "--python".to_owned(),
        "3.13".to_owned(),
        "--with".to_owned(),
        format!("mutmut=={version}"),
        "mutmut".to_owned(),
        "run".to_owned(),
        "--paths-to-mutate".to_owned(),
        sources.join(","),
        "--tests-dir".to_owned(),
        tests_dir,
        // NOT --no-progress: observed live (2.5.1, e2e run 2026-07-18),
        // that flag suppresses the counts line entirely — the spinner's
        // final `\r`-delimited segment is the only counts report.
        "--simple-output".to_owned(),
    ];
    eprintln!("gate mutate: mutmut@{version} ({}) …", scope_word(scope));
    let output = exec(&argv, &dir, &[])?;
    let code = output.status.code().unwrap_or(-1);
    // mutmut's exit code is a bitmask: 1 error, 2 survived, 4 timeout,
    // 8 suspicious. Any code with the error bit (or out of range) failed.
    if !(0..=14).contains(&code) || code & 1 == 1 {
        return Err(GateError::ToolFailed {
            tool: "mutmut".to_owned(),
            code: code.to_string(),
            output: tail(
                &format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
                30,
            ),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tally = parse_mutmut_counts(&stdout)?;
    // Aggregate survivors only: mutmut 2's per-mutant browser is broken on
    // python ≥ 3.13 (module docs) — file/line detail is a known v1 limit.
    let findings = if tally.missed > 0 {
        vec![Finding {
            gate: "mutate",
            tool: "mutmut",
            rule: "survived-mutant".to_owned(),
            file: cwd.map_or_else(|| sources.join(","), str::to_owned),
            line: None,
            message: format!(
                "{} mutant(s) survived in {} — run `mutmut results` in the \
                 stack for detail",
                tally.missed,
                sources.join(",")
            ),
            severity: Severity::Medium,
        }]
    } else {
        Vec::new()
    };
    Ok(StackRun {
        tool: "mutmut",
        tally,
        findings,
        note: None,
    })
}

/// Parse the final mutmut progress segment:
/// `⠼ 2/2  KILLED 0  TIMEOUT 0  SUSPICIOUS 0  SURVIVED 2  SKIPPED 0`
/// (observed live against mutmut 2.5.1, e2e run 2026-07-18). The spinner
/// separates progress updates with bare `\r` (not `\r\n`), so segments are
/// split on both; `--simple-output`'s legend also contains the KILLED
/// keyword without counts, so the LAST segment that fully parses wins.
/// Suspicious mutants count against the score (not proven killed) but are
/// not reported as survivors.
fn parse_mutmut_counts(stdout: &str) -> Result<Tally, GateError> {
    let grab = |line: &str, key: &str| -> Option<usize> {
        line.split_whitespace()
            .skip_while(|w| *w != key)
            .nth(1)
            .and_then(|n| n.parse().ok())
    };
    stdout
        .split(['\n', '\r'])
        .rev()
        .find_map(|line| {
            Some(Tally {
                caught: grab(line, "KILLED")?,
                timeout: grab(line, "TIMEOUT")?,
                missed: grab(line, "SURVIVED")? + grab(line, "SUSPICIOUS")?,
                unviable: grab(line, "SKIPPED")?,
            })
        })
        .ok_or_else(|| GateError::Parse {
            tool: "mutmut",
            detail: "no KILLED/SURVIVED counts segment in mutmut output".to_owned(),
        })
}

// --------------------------------------------------------------- typescript

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutmut_counts_parse_from_the_progress_line() {
        // Observed live against mutmut 2.5.1 (scratch project).
        let stdout = "1. Running tests without mutations\nDone\n\n2. Checking mutants\n\u{283c} 3/3  KILLED 1  TIMEOUT 0  SUSPICIOUS 0  SURVIVED 2  SKIPPED 0\n";
        let tally = parse_mutmut_counts(stdout).expect("parses");
        assert_eq!(tally.caught, 1);
        assert_eq!(tally.missed, 2);
        assert_eq!(tally.unviable, 0);
        assert!(parse_mutmut_counts("no counts here").is_err());
    }

    #[test]
    fn mutmut_counts_parse_the_real_cr_delimited_spinner() {
        // Captured live from the e2e run (mutmut 2.5.1, 2026-07-18):
        // --simple-output prints a KILLED legend line without counts, and
        // the spinner separates stale progress segments with bare \r —
        // only the final segment carries the true tally.
        let stdout = "Legend for output:\nKILLED Killed mutants.   The goal is for everything to end up in this bucket.\n\n2. Checking mutants\n\u{2838} 1/2  KILLED 0  TIMEOUT 0  SUSPICIOUS 0  SURVIVED 1  SKIPPED 0\r\u{283c} 2/2  KILLED 0  TIMEOUT 0  SUSPICIOUS 0  SURVIVED 2  SKIPPED 0\n";
        let tally = parse_mutmut_counts(stdout).expect("parses");
        assert_eq!(tally.caught, 0);
        assert_eq!(tally.missed, 2, "the FINAL segment wins, not a stale one");
        assert_eq!(tally.unviable, 0);
    }
}

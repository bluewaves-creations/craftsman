//! `[gates.qa]` — project-declared QA commands as first-class gates
//! (ADR-006 §5). They run inside check-all after the static set —
//! uncached (a command's inputs are unknowable) and fail-fast like the
//! static gates — and therefore inside the commit gate and the
//! `Verified-by:` trailer. Strict-only: a command verdict has no findings
//! to fingerprint, and a qa gate is never a substitute for `verify`.

use std::path::Path;

use super::GateError;
use super::check_all::{GateSummary, GateVerdict};
use crate::config::Config;

/// Run every declared qa gate, appending its row to `gates`. Skipped
/// entirely when an earlier gate is already red.
///
/// # Errors
/// [`GateError::QaSpawn`] / [`GateError::QaMissing`] — a gate that cannot
/// run is exit-3 territory, never a verdict.
pub(super) fn run_all(
    root: &Path,
    config: &Config,
    gates: &mut Vec<GateSummary>,
) -> Result<(), GateError> {
    if gates.iter().any(|g| g.verdict == GateVerdict::Red) {
        return Ok(());
    }
    for (name, qa) in &config.gates.qa {
        let summary = run_one(root, name, &qa.command)?;
        let green = summary.verdict == GateVerdict::Green;
        gates.push(summary);
        if !green {
            break;
        }
    }
    Ok(())
}

/// One declared `[gates.qa]` command: `sh -c` in the project root, exit 0
/// green, any other exit red — except 127 (command not found), which is a
/// misdeclared gate and therefore exit-3 territory, never a red verdict.
fn run_one(root: &Path, name: &str, command: &str) -> Result<GateSummary, GateError> {
    eprintln!("gate qa:{name}: {command} …");
    let output = std::process::Command::new("sh")
        .args(["-c", command])
        .current_dir(root)
        .output()
        .map_err(|source| GateError::QaSpawn {
            name: name.to_owned(),
            command: command.to_owned(),
            source,
        })?;
    // Command output is progress, not verdict: forward it to stderr.
    eprint!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    let code = output.status.code();
    if code == Some(127) {
        return Err(GateError::QaMissing {
            name: name.to_owned(),
            command: command.to_owned(),
        });
    }
    let (verdict, detail) = if output.status.success() {
        (GateVerdict::Green, "command exited 0".to_owned())
    } else {
        (
            GateVerdict::Red,
            format!(
                "command exited {}",
                code.map_or_else(|| "on signal".to_owned(), |c| c.to_string())
            ),
        )
    };
    Ok(GateSummary {
        gate: format!("qa:{name}"),
        mode: "strict".to_owned(),
        verdict,
        detail,
    })
}

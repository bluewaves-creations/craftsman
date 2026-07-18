//! Baseline management: recording (`gate baseline`), the status table,
//! and the strict flip — the operations layered over the snapshot model
//! in the parent module.

use std::path::Path;

use serde::Serialize;

use super::super::adapter::{self, BaselineKind};
use super::super::{Finding, GateError, exec, tail, tools};
use super::{Baseline, DIR, NativeBaseline, load, path, save};
use crate::config::{Config, GateMode};

/// `gate baseline lint` — record the lint gate's debt (Batch 9a).
///
/// The fingerprint snapshot takes snapshot-kind tools only; when a swift
/// stack is configured, `SwiftLint` additionally writes its native
/// baseline (`swiftlint lint --write-baseline
/// .craftsman/baselines/swiftlint.json`, the file baseline-mode runs
/// later pass back via `--baseline`). Native-tool findings stay out of
/// the fingerprint snapshot (they are diffed tool-side; recording them
/// twice would double-count the debt).
///
/// # Errors
/// Tool resolution/spawn/parse failures; baseline write failures.
pub fn record_lint(root: &Path, config: &Config) -> Result<Baseline, GateError> {
    let outcome = super::super::lint::run(root, config, None, GateMode::Strict)?;
    let snapshot: Vec<Finding> = outcome
        .findings
        .into_iter()
        .filter(|f| adapter::tool(f.tool).is_none_or(|t| t.baseline == BaselineKind::Snapshot))
        .collect();
    let mut base = Baseline::record("lint", &snapshot);
    base.swiftlint = write_swiftlint_baseline(root, config)?;
    save(root, &base)?;
    Ok(base)
}

/// Run `swiftlint lint --write-baseline` for the swift stack, returning
/// the recorded debt. `None` when no swift stack is configured.
pub(super) fn write_swiftlint_baseline(
    root: &Path,
    config: &Config,
) -> Result<Option<NativeBaseline>, GateError> {
    if !config.project.stacks.iter().any(|s| s == "swift") {
        return Ok(None);
    }
    let tool = adapter::tool("swiftlint").expect("swiftlint is in the adapter table");
    let resolved = tools::resolve(tool, &super::super::lint::pinned_version(config, tool))?;
    let native = path(root, "swiftlint");
    if let Some(parent) = native.parent() {
        std::fs::create_dir_all(parent).map_err(|source| GateError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let cwd = config.verify.stack("swift").and_then(|s| s.cwd.as_deref());
    let dir = cwd.map_or_else(|| root.to_path_buf(), |c| root.join(c));

    let mut argv = resolved.argv.clone();
    argv.extend(tool.base_args.iter().map(|s| (*s).to_owned()));
    argv.push("--write-baseline".to_owned());
    argv.push(native.to_string_lossy().into_owned());
    eprintln!("gate lint: swiftlint --write-baseline ({}) …", resolved.via);
    let output = exec(&argv, &dir, &[])?;
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let findings = adapter::parse(tool, &stdout, &String::from_utf8_lossy(&output.stderr))?;
    if !tool.success_codes.contains(&code) && findings.is_empty() {
        return Err(GateError::ToolFailed {
            tool: format!("swiftlint ({})", argv.join(" ")),
            code: code.to_string(),
            output: tail(&stdout, 30),
        });
    }
    Ok(Some(NativeBaseline {
        file: DIR.to_owned() + "/swiftlint.json",
        count: findings.len(),
    }))
}

/// One row of `craftsman gate status`.
#[derive(Debug, Serialize)]
pub struct StatusRow {
    pub gate: &'static str,
    pub mode: String,
    pub baseline: usize,
    pub recorded_at: Option<String>,
    pub last_ratchet: Option<String>,
}

/// The nine gates in surface order with their configured mode and baseline
/// state.
///
/// # Errors
/// Baseline read failures.
pub fn status(root: &Path, config: &Config) -> Result<Vec<StatusRow>, GateError> {
    let gates = config.gates.by_name();
    let mut rows = Vec::new();
    for (gate, mode) in gates {
        let mode_str = mode.map_or_else(|| "off".to_owned(), |m| m.to_string());
        let base = load(root, gate)?;
        rows.push(StatusRow {
            gate,
            mode: mode_str,
            baseline: base.as_ref().map_or(0, Baseline::count),
            recorded_at: base.as_ref().map(|b| b.recorded_at.clone()),
            last_ratchet: base.and_then(|b| b.last_ratchet),
        });
    }
    Ok(rows)
}

/// Flip a gate to strict in `craftsman.toml` — only when its baseline debt
/// is zero. Returns `Err(count)` (the refusal, exit 1 at the command
/// layer) when debt remains.
///
/// The edit is textual and minimal (the `toml` crate does not preserve
/// formatting and `toml_edit` is not a vetted dependency here); the result
/// is re-parsed through [`Config::from_toml`] before being written.
///
/// # Errors
/// [`GateError`] on IO or when the edited config fails validation.
pub fn flip_strict(root: &Path, gate: &str) -> Result<Result<(), usize>, GateError> {
    let debt = load(root, gate)?.as_ref().map_or(0, Baseline::count);
    if debt > 0 {
        return Ok(Err(debt));
    }
    let config_path = root.join(crate::config::FILE_NAME);
    let text = std::fs::read_to_string(&config_path).map_err(|source| GateError::Io {
        path: config_path.clone(),
        source,
    })?;
    let edited = set_gate_mode(&text, gate);
    // Prove the edit before writing it.
    Config::from_toml(&edited, &config_path)?;
    std::fs::write(&config_path, edited).map_err(|source| GateError::Io {
        path: config_path,
        source,
    })?;
    Ok(Ok(()))
}

/// Rewrite (or insert) `<gate> = "strict"` inside `[gates]`.
pub(super) fn set_gate_mode(text: &str, gate: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut in_gates = false;
    let mut gates_header: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_gates = trimmed == "[gates]";
            if in_gates {
                gates_header = Some(i);
            }
            continue;
        }
        if in_gates
            && trimmed
                .split_once('=')
                .is_some_and(|(key, _)| key.trim() == gate)
        {
            lines[i] = format!("{gate} = \"strict\"");
            return join_lines(&lines);
        }
    }
    if let Some(header) = gates_header {
        lines.insert(header + 1, format!("{gate} = \"strict\""));
    } else {
        if !lines.last().is_none_or(|l| l.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push("[gates]".to_owned());
        lines.push(format!("{gate} = \"strict\""));
    }
    join_lines(&lines)
}

pub(super) fn join_lines(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_flip_edits_only_the_gate_line() {
        let text = "[project]\nname = \"x\"\n\n[gates]\nverify = \"strict\"\nlint = \"baseline\"\n";
        let edited = set_gate_mode(text, "lint");
        assert!(edited.contains("lint = \"strict\""));
        assert!(edited.contains("verify = \"strict\""));
        assert!(edited.contains("name = \"x\""));

        let inserted = set_gate_mode("[project]\nname = \"x\"\n", "lint");
        assert!(inserted.contains("[gates]\nlint = \"strict\""));
    }
}

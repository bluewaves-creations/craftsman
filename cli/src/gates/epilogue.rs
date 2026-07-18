//! The shared gate epilogue — every gate's findings route through
//! [`finish`], which enforces `[gates] exclude` centrally and applies the
//! enforcement mode (baseline vs strict) against the committed snapshot.

use std::path::Path;

use super::adapter::{self, BaselineKind};
use super::{Finding, GateError, GateOutcome, baseline};
use crate::config::{Config, GateMode};

/// The run context every gate shares — bundled so the epilogue signature
/// stays honest instead of sprouting positional arguments.
pub struct Epilogue<'a> {
    pub root: &'a Path,
    pub config: &'a Config,
    pub gate: &'static str,
    pub changed: Option<&'a [String]>,
    pub mode: GateMode,
}

/// Shared gate epilogue: drop findings under `[gates] exclude`, apply the
/// mode (baseline vs strict), and assemble the outcome. Every gate routes
/// through here — the exclusion is enforced centrally, once.
pub fn finish(
    ctx: &Epilogue<'_>,
    mut findings: Vec<Finding>,
    mut notes: Vec<String>,
    tools_ran: Vec<&'static str>,
) -> Result<GateOutcome, GateError> {
    let (root, gate, changed, mode) = (ctx.root, ctx.gate, ctx.changed, ctx.mode);
    let exclude = &ctx.config.gates.exclude;
    if !exclude.is_empty() {
        let before = findings.len();
        findings.retain(|f| !super::scope::is_excluded(exclude, &f.file));
        let dropped = before - findings.len();
        if dropped > 0 {
            notes.push(format!(
                "{gate}: {dropped} finding(s) under [gates] exclude scope — dropped"
            ));
        }
    }
    let (blocking, baselined, ratchet) = match mode {
        GateMode::Baseline => {
            // Internal gate tools (health, arch, mutate) are not in the
            // adapter table — they have no native baseline mechanism, so
            // unknown names default to the unified snapshot.
            let snapshot_tools: Vec<&'static str> = tools_ran
                .iter()
                .copied()
                .filter(|name| {
                    adapter::tool(name).is_none_or(|t| t.baseline == BaselineKind::Snapshot)
                })
                .collect();
            let (snapshot, native): (Vec<Finding>, Vec<Finding>) = findings
                .clone()
                .into_iter()
                .partition(|f| snapshot_tools.contains(&f.tool));
            let applied =
                baseline::apply(root, gate, snapshot, &snapshot_tools, changed.is_none())?;
            // Native-baseline tools already diffed tool-side: everything
            // they still report is new.
            let mut blocking = applied.new_findings;
            blocking.extend(native);
            if !applied.had_baseline && !blocking.is_empty() {
                // Baseline mode with nothing recorded: the findings block
                // only because the debt was never snapshotted. Name the
                // remedy instead of leaving the human to guess (the
                // craftsman-web dogfood hit exactly this, ledger 4b).
                notes.push(format!(
                    "{gate}: mode is baseline but no baseline is recorded — accept \
                     this inherited debt explicitly with `craftsman gate baseline \
                     {gate}`, or fix the findings"
                ));
            }
            (blocking, applied.baselined, applied.ratchet)
        }
        GateMode::Strict | GateMode::Off => (findings.clone(), 0, None),
    };
    if mode == GateMode::Off {
        notes.push(format!(
            "gate {gate} is off in craftsman.toml — this direct run enforced strict"
        ));
    }
    Ok(GateOutcome {
        gate,
        mode: if mode == GateMode::Off {
            GateMode::Strict
        } else {
            mode
        },
        findings,
        blocking,
        baselined,
        ratchet,
        notes,
        tools_ran,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::Severity;

    #[test]
    fn finish_drops_findings_under_exclude_scope() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config = Config::from_toml(
            "[project]\nname = \"x\"\nstacks = [\"rust\"]\n\
             [gates]\nexclude = [\"spikes/**\"]\n",
            Path::new("craftsman.toml"),
        )
        .expect("parses");
        let f = |file: &str| Finding {
            gate: "lint",
            tool: "clippy",
            rule: "r".to_owned(),
            file: file.to_owned(),
            line: None,
            message: "m".to_owned(),
            severity: Severity::Medium,
        };
        let out = finish(
            &Epilogue {
                root: tmp.path(),
                config: &config,
                gate: "lint",
                changed: None,
                mode: GateMode::Strict,
            },
            vec![f("spikes/s2/lib.rs"), f("cli/src/a.rs")],
            Vec::new(),
            vec!["clippy"],
        )
        .expect("finish");
        assert_eq!(out.blocking.len(), 1, "{:?}", out.blocking);
        assert_eq!(out.blocking[0].file, "cli/src/a.rs");
        assert_eq!(out.findings.len(), 1, "dropped from visibility too");
        assert!(
            out.notes.iter().any(|n| n.contains("exclude scope")),
            "{:?}",
            out.notes
        );
    }
}

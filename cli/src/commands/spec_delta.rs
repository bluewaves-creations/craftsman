//! `craftsman spec lint --delta` and `craftsman spec merge-delta` — the
//! delta workflow's command layer.

use std::path::Path;

use craftsman::spec::{self, Severity};

use super::spec::{count_errors, print_findings};
use super::{EXIT_EMPTY_SELECTION, EXIT_PASS, EXIT_VERIFICATION_FAILURE, load};

/// Lint the delta file against the executed spec: the delta's scenario
/// names and the lint findings. `None` = no delta file (the exit-4
/// message is already printed).
fn delta_lint_findings(
    spec_path: &Path,
    delta: &Path,
) -> anyhow::Result<Option<(Vec<String>, Vec<spec::Finding>)>> {
    if !delta.is_file() {
        eprintln!(
            "spec: no SPEC.delta.md next to the executed spec — \
             nothing to lint or merge (exit 4)"
        );
        return Ok(None);
    }
    let feature = spec::parse_spec(spec_path)?;
    let names: Vec<String> = spec::inventory(&feature)
        .into_iter()
        .map(|e| e.scenario)
        .collect();
    match spec::parse_spec(delta) {
        Ok(delta_feature) => {
            let delta_names = spec::inventory(&delta_feature)
                .into_iter()
                .map(|e| e.scenario)
                .collect();
            let findings = spec::lint_delta(&delta_feature, &names);
            Ok(Some((delta_names, findings)))
        }
        Err(err @ (spec::SpecError::Read { .. } | spec::SpecError::Write { .. })) => {
            Err(err.into())
        }
        Err(spec::SpecError::Parse { message, .. }) => Ok(Some((
            Vec::new(),
            vec![spec::Finding {
                severity: Severity::Error,
                rule: "parse-error",
                line: 0,
                message,
            }],
        ))),
    }
}

pub fn spec_lint_delta(json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let spec_path = loaded.root.join(&loaded.config.project.spec);
    let delta = spec::delta_path(&spec_path);
    let Some((delta_names, findings)) = delta_lint_findings(&spec_path, &delta)? else {
        return Ok(EXIT_EMPTY_SELECTION);
    };
    let errors = count_errors(&findings);
    let warnings = findings.len() - errors;
    if json {
        let doc = serde_json::json!({
            "delta": "SPEC.delta.md",
            "scenarios": delta_names,
            "findings": findings,
            "errors": errors,
            "warnings": warnings,
        });
        println!("{doc:#}");
    } else {
        for name in &delta_names {
            eprintln!("  delta  {name}");
        }
        print_findings(&findings);
        println!(
            "spec lint --delta: {errors} error(s), {warnings} warning(s) in \
             SPEC.delta.md — nothing admitted to the executed set"
        );
    }
    Ok(if errors > 0 {
        EXIT_VERIFICATION_FAILURE
    } else {
        EXIT_PASS
    })
}

pub fn spec_merge_delta(json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let spec_rel = loaded.config.project.spec.clone();
    let spec_path = loaded.root.join(&spec_rel);
    let delta = spec::delta_path(&spec_path);
    let Some((_, findings)) = delta_lint_findings(&spec_path, &delta)? else {
        return Ok(EXIT_EMPTY_SELECTION);
    };
    let errors = count_errors(&findings);
    if errors > 0 {
        print_findings(&findings);
        eprintln!(
            "merge-delta refused: SPEC.delta.md has {errors} lint error(s) — \
             the delta file is kept; fix and re-run (exit 1)"
        );
        if json {
            let doc = serde_json::json!({ "merged": 0, "findings": findings });
            println!("{doc:#}");
        }
        return Ok(EXIT_VERIFICATION_FAILURE);
    }
    let moved = spec::merge_delta(&spec_path, &delta)?;
    eprintln!(
        "merge-delta: folded {moved} scenario(s) into {spec_rel} and removed \
         SPEC.delta.md — the write is mediated, never committed; commit the \
         merge yourself"
    );
    if json {
        let doc = serde_json::json!({ "merged": moved, "spec": spec_rel });
        println!("{doc:#}");
    }
    Ok(EXIT_PASS)
}

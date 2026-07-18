//! `craftsman perf|a11y|visual` — thin orchestration over runtime tools.
//!
//! These gates run *user-land* configurations; craftsman owns invocation,
//! refusal, and verdict normalization — never the test content:
//!
//! - **perf** — `[perf] lighthouse-config` → `bunx @lhci/cli autorun`
//!   (failed assertions from `.lighthouseci/assertion-results.json`), OR
//!   `[perf] k6-script` → the pinned k6 binary with `--summary-export`
//!   (crossed thresholds from the summary metrics).
//! - **a11y** — web path: `bunx playwright test <[a11y] test-glob>` with
//!   the JSON reporter (axe-based specs are user-land); apple path
//!   (`[a11y] scheme` + `ui-test-target`, Batch 9a): `xcodebuild test
//!   -only-testing:<target>` running user-land `XCUITest` audits, failed
//!   tests read from the result bundle via the verify xcodebuild adapter.
//! - **visual** — same runner as web a11y, `[visual] test-glob`
//!   (screenshot specs).
//!
//! A gate whose config section is absent **refuses** with a clear message
//! (exit 3: "not configured — see --help") — an enabled-but-unconfigured
//! runtime gate must never pass silently.
//!
//! Playwright resolves through `bunx` (house rule: bun, never npx), which
//! prefers the project's locally installed playwright — the project's
//! lockfile is the version pin. `[gates.tools] playwright` forces a
//! version when set.

mod perf;
mod ui;

use std::path::Path;

use perf::run_perf;
use ui::{run_playwright, run_xcuitest_audit};

use super::{Finding, GateError, GateOutcome, epilogue};
use crate::config::{Config, GateMode};

/// Pinned Lighthouse CI version (`[gates.tools] lhci` overrides).
pub(super) const LHCI_VERSION: &str = "0.15.1";

/// Run one runtime gate (`perf`, `a11y`, or `visual`).
///
/// # Errors
/// [`GateError::NotConfigured`] when the gate's config section is absent;
/// tool spawn/parse failures.
pub fn run(
    root: &Path,
    config: &Config,
    gate: &'static str,
    changed: Option<&[String]>,
    mode: GateMode,
) -> Result<GateOutcome, GateError> {
    let mut notes: Vec<String> = Vec::new();
    if changed.is_some() {
        notes.push(format!(
            "{gate}: --changed never narrows a runtime gate — running the \
             configured suite in full"
        ));
    }
    let (findings, tool): (Vec<Finding>, &'static str) = match gate {
        "perf" => run_perf(root, config)?,
        "a11y" => {
            let a11y = config.a11y.as_ref().ok_or_else(|| {
                not_configured("a11y", "test-glob (web) or scheme + ui-test-target (apple)")
            })?;
            match (&a11y.test_glob, &a11y.scheme, &a11y.ui_test_target) {
                (Some(glob), None, None) => {
                    (run_playwright(root, config, "a11y", glob)?, "playwright")
                }
                (None, Some(scheme), Some(target)) => (
                    run_xcuitest_audit(root, scheme, target, a11y.destination.as_deref())?,
                    "xcodebuild",
                ),
                // Config validation enforces web XOR apple, each complete.
                _ => unreachable!("[a11y] validation admits exactly one complete path"),
            }
        }
        "visual" => {
            let glob = config
                .visual
                .as_ref()
                .map(|c| c.test_glob.clone())
                .ok_or_else(|| not_configured("visual", "test-glob"))?;
            (run_playwright(root, config, "visual", &glob)?, "playwright")
        }
        other => unreachable!("not a runtime gate: {other}"),
    };
    epilogue::finish(
        &epilogue::Epilogue {
            root,
            config,
            gate,
            changed,
            mode,
        },
        findings,
        notes,
        vec![tool],
    )
}

pub(super) fn not_configured(gate: &'static str, key: &str) -> GateError {
    GateError::NotConfigured {
        gate,
        hint: format!("add [{gate}] {key} = \"…\" to craftsman.toml"),
    }
}

// --------------------------------------------------------------------- perf

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconfigured_runtime_gates_refuse_loudly() {
        let config = crate::config::Config::from_toml(
            "[project]\nname = \"x\"\nstacks = [\"rust\"]\n",
            Path::new("craftsman.toml"),
        )
        .expect("parses");
        for gate in ["perf", "a11y", "visual"] {
            let err = run(Path::new("."), &config, gate, None, GateMode::Strict)
                .expect_err("must refuse");
            assert!(
                matches!(err, GateError::NotConfigured { gate: g, .. } if g == gate),
                "{err}"
            );
            assert!(err.to_string().contains("not configured"), "{err}");
        }
    }

    #[test]
    fn perf_with_an_empty_section_refuses() {
        let config = crate::config::Config::from_toml(
            "[project]\nname = \"x\"\nstacks = [\"rust\"]\n[perf]\n",
            Path::new("craftsman.toml"),
        )
        .expect("parses");
        let err =
            run(Path::new("."), &config, "perf", None, GateMode::Strict).expect_err("must refuse");
        assert!(matches!(err, GateError::NotConfigured { .. }), "{err}");
    }
}

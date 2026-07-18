//! The gate family: direct gate invocations, `mutate`, `check-all`,
//! `gate status|baseline|strict`, and `doctor`.

use clap::Subcommand;

use craftsman::gates::{self, GateOutcome, baseline, check_all};

use super::{EXIT_PASS, EXIT_VERIFICATION_FAILURE, cwd, load};

#[derive(Subcommand)]
pub enum GateCommand {
    /// Per-gate mode, baseline count, and ratchet history
    Status {
        /// Emit the rows as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Record (or refresh) a gate's baseline — the brownfield Phase 2
    /// move: existing findings become the committed debt snapshot; the
    /// gate then fails only on new findings.
    ///
    /// swiftlint and semgrep use their native mechanisms (baseline file /
    /// baseline commit ref); every other tool lands in the unified
    /// fingerprint snapshot at .craftsman/baselines/<gate>.json.
    ///
    /// Exit codes: 0 recorded · 2 usage error · 3 unsupported gate or
    /// tool failure.
    Baseline {
        /// The gate to record (lint | security | health | arch)
        gate: String,
        /// Emit {gate, count, recorded-at} as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Flip a gate to strict in craftsman.toml — only when its baseline
    /// debt is zero (exit 1 with the count otherwise).
    Strict {
        /// The gate to flip
        gate: String,
        /// Emit {gate, flipped, remaining} as JSON on stdout
        #[arg(long)]
        json: bool,
    },
}

/// Dispatch `craftsman gate <status|baseline|strict>`.
pub fn run(command: &GateCommand) -> anyhow::Result<i32> {
    match command {
        GateCommand::Status { json } => gate_status_cmd(*json),
        GateCommand::Baseline { gate, json } => gate_baseline_cmd(gate, *json),
        GateCommand::Strict { gate, json } => gate_strict_cmd(gate, *json),
    }
}

/// Shared command flow for the direct gate invocations.
pub fn gate_cmd(gate: &'static str, changed: bool, json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let config = &loaded.config;
    let root = &loaded.root;

    // Direct invocation runs even when the gate is off in config —
    // explicitly asking for a gate is not a config lookup — enforcing the
    // configured mode when present, strict otherwise.
    let mode = config
        .gates
        .mode(gate)
        .unwrap_or(craftsman::config::GateMode::Strict);
    let changed_set = if changed {
        Some(gates::changed_files(root)?)
    } else {
        None
    };
    let outcome = match gate {
        "lint" => gates::lint::run(root, config, changed_set.as_deref(), mode)?,
        "security" => gates::security::run(root, config, changed_set.as_deref(), mode)?,
        "arch" => gates::arch::run(root, config, changed_set.as_deref(), mode)?,
        "health" => gates::health::run(root, config, changed_set.as_deref(), mode)?,
        "perf" | "a11y" | "visual" => {
            gates::runtime::run(root, config, gate, changed_set.as_deref(), mode)?
        }
        _ => unreachable!("only gate subcommands route here"),
    };
    print_outcome(&outcome, json);
    Ok(if outcome.passed() {
        EXIT_PASS
    } else {
        EXIT_VERIFICATION_FAILURE
    })
}

/// `craftsman mutate` — diff-scoped by default; `--all` (guarded by
/// `--yes-slow` at the parser level) runs everything.
pub fn mutate_cmd(all: bool, json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let mode = loaded
        .config
        .gates
        .mode("mutate")
        .unwrap_or(craftsman::config::GateMode::Strict);
    let scope = if all {
        gates::mutate::Scope::All
    } else {
        gates::mutate::Scope::Diff
    };
    let outcome = gates::mutate::run(&loaded.root, &loaded.config, scope, mode)?;
    print_outcome(&outcome, json);
    Ok(if outcome.passed() {
        EXIT_PASS
    } else {
        EXIT_VERIFICATION_FAILURE
    })
}

fn print_outcome(outcome: &GateOutcome, json: bool) {
    for note in &outcome.notes {
        eprintln!("note: {note}");
    }
    if let Some(ratchet) = &outcome.ratchet {
        eprintln!("{ratchet}");
    }
    for f in &outcome.findings {
        let blocking = outcome
            .blocking
            .iter()
            .any(|b| baseline::fingerprint(b) == baseline::fingerprint(f));
        let mark = if blocking { "FAIL" } else { "base" };
        let line = f.line.map_or_else(String::new, |l| format!(":{l}"));
        eprintln!(
            "  {mark}  {}{line}  [{}/{}] {} ({})",
            f.file, f.tool, f.rule, f.message, f.severity
        );
    }
    eprintln!(
        "gate {}: {} — mode {}, {} tool(s) ran",
        outcome.gate,
        outcome.detail(),
        outcome.mode,
        outcome.tools_ran.len()
    );
    if json {
        let doc = serde_json::json!({
            "gate": outcome.gate,
            "mode": outcome.mode,
            "passed": outcome.passed(),
            "findings": outcome.findings,
            "blocking": outcome.blocking.len(),
            "baselined": outcome.baselined,
            "tools": outcome.tools_ran,
            "notes": outcome.notes,
        });
        println!("{doc:#}");
    }
}

pub fn check_all_cmd(changed: bool, json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let report = check_all::run(&loaded.root, &loaded.config, changed)?;

    eprintln!("check-all:");
    for g in &report.gates {
        let mark = match g.verdict {
            check_all::GateVerdict::Green => "ok  ",
            check_all::GateVerdict::CachedGreen => "ok* ",
            check_all::GateVerdict::Off => "off ",
            check_all::GateVerdict::Red => "FAIL",
        };
        eprintln!("  {mark}  {:<9} {:<9} {}", g.gate, g.mode, g.detail);
    }
    if json {
        let doc = serde_json::json!({
            "passed": report.passed(),
            "changed": changed,
            "gates": report.gates,
        });
        println!("{doc:#}");
    }
    Ok(if report.passed() {
        EXIT_PASS
    } else {
        EXIT_VERIFICATION_FAILURE
    })
}

fn gate_status_cmd(json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let rows = baseline::status(&loaded.root, &loaded.config)?;
    eprintln!(
        "{:<9} {:<9} {:>8}  {:<20} last ratchet",
        "gate", "mode", "baseline", "recorded"
    );
    for r in &rows {
        eprintln!(
            "{:<9} {:<9} {:>8}  {:<20} {}",
            r.gate,
            r.mode,
            r.baseline,
            r.recorded_at.as_deref().unwrap_or("-"),
            r.last_ratchet.as_deref().unwrap_or("-"),
        );
    }
    if json {
        let doc = serde_json::json!({ "gates": rows });
        println!("{doc:#}");
    }
    Ok(EXIT_PASS)
}

fn gate_baseline_cmd(gate: &str, json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let root = &loaded.root;
    let config = &loaded.config;
    let strict = craftsman::config::GateMode::Strict;
    let recorded = match gate {
        // lint owns its recording: snapshot + the SwiftLint native
        // baseline when a swift stack is configured (Batch 9a).
        "lint" => gates::baseline::record_lint(root, config)?,
        "health" | "arch" => {
            let outcome = match gate {
                "health" => gates::health::run(root, config, None, strict)?,
                "arch" => gates::arch::run(root, config, None, strict)?,
                _ => unreachable!("matched above"),
            };
            let base = baseline::Baseline::record(gate, &outcome.findings);
            baseline::save(root, &base)?;
            base
        }
        "security" => gates::security::record_baseline(root, config)?,
        other => {
            return Err(gates::GateError::UnsupportedGate {
                gate: other.to_owned(),
            }
            .into());
        }
    };
    eprintln!(
        "gate {gate}: baseline recorded — {} finding(s) snapshotted at {} \
         (commit .craftsman/baselines/; the gate now fails only on new findings)",
        recorded.count(),
        recorded.recorded_at
    );
    if json {
        let doc = serde_json::json!({
            "gate": gate,
            "count": recorded.count(),
            "recorded-at": recorded.recorded_at,
        });
        println!("{doc:#}");
    }
    Ok(EXIT_PASS)
}

fn gate_strict_cmd(gate: &str, json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let result = baseline::flip_strict(&loaded.root, gate)?;
    if json {
        let doc = serde_json::json!({
            "gate": gate,
            "flipped": result.is_ok(),
            "remaining": result.as_ref().err().copied().unwrap_or(0),
        });
        println!("{doc:#}");
    }
    match result {
        Ok(()) => {
            eprintln!("gate {gate}: flipped to strict in craftsman.toml (baseline debt is zero)");
            Ok(EXIT_PASS)
        }
        Err(count) => {
            eprintln!(
                "gate {gate}: refusing the strict flip — the baseline still holds \
                 {count} finding(s); ratchet it to zero first"
            );
            Ok(EXIT_VERIFICATION_FAILURE)
        }
    }
}

pub fn doctor_cmd(json: bool) -> anyhow::Result<i32> {
    let checks = craftsman::doctor::run(&cwd()?);
    let passed = checks.iter().all(|c| c.passed);

    for c in &checks {
        let mark = if c.passed { "ok  " } else { "FAIL" };
        eprintln!("{mark}  {:<10}  {}", c.name, c.detail);
    }
    eprintln!(
        "doctor: {}/{} checks passed",
        checks.iter().filter(|c| c.passed).count(),
        checks.len()
    );
    if json {
        let doc = serde_json::json!({ "passed": passed, "checks": checks });
        println!("{doc:#}");
    }
    Ok(if passed {
        EXIT_PASS
    } else {
        EXIT_VERIFICATION_FAILURE
    })
}

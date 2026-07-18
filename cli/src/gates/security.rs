//! `craftsman security` — gitleaks (git history), semgrep (pinned registry
//! ruleset, ERROR severity), osv-scanner (lockfiles, offline databases),
//! run in parallel and merged into one finding set.
//!
//! Network discipline: semgrep's ruleset and osv's vulnerability databases
//! are fetched once at first use into `~/.craftsman/tools/`; every verdict
//! run afterwards is offline. `--changed` never narrows this gate —
//! narrowing a secret or vulnerability scan to a diff hides standing risk
//! (the gate cache in check-all is the fast path instead).
//!
//! Baselines are hybrid: semgrep natively (recorded baseline commit ref →
//! `--baseline-commit`, so only findings introduced after it surface);
//! gitleaks + osv via the unified fingerprint snapshot.

use std::path::Path;

use super::adapter::{self, GateTool};
use super::{Finding, GateError, GateOutcome, Severity, baseline, epilogue, exec, tail, tools};
use crate::config::{Config, GateMode};

/// Lockfile names osv-scanner is pointed at (tracked files only).
const LOCKFILES: &[&str] = &[
    "Cargo.lock",
    "uv.lock",
    "poetry.lock",
    "requirements.txt",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lock",
    "Package.resolved",
    "Gemfile.lock",
    "go.mod",
];

/// Run the security gate.
///
/// # Errors
/// [`GateError`] when any scanner cannot be resolved, spawned, or parsed.
///
/// # Panics
/// Never in practice — scanner threads return their errors instead of
/// panicking.
pub fn run(
    root: &Path,
    config: &Config,
    changed: Option<&[String]>,
    mode: GateMode,
) -> Result<GateOutcome, GateError> {
    let mut notes: Vec<String> = Vec::new();
    if changed.is_some() {
        notes.push(
            "security: --changed never narrows this gate (secrets and vulnerable \
             dependencies are standing risk) — running in full"
                .to_owned(),
        );
    }
    let threshold = config.gates.security_threshold.unwrap_or(Severity::High);

    // Resolve serially (installs may download); run in parallel.
    let gitleaks = resolved(config, "gitleaks")?;
    let semgrep = resolved(config, "semgrep")?;
    let osv = resolved(config, "osv-scanner")?;
    let ruleset = tools::ensure_download(
        &format!("semgrep-rules@{}", pinned(config, "semgrep")),
        "default.yaml",
        "https://semgrep.dev/c/p/default",
    )?;
    let semgrep_ref = match mode {
        GateMode::Baseline => baseline::load(root, "security")?
            .and_then(|b| b.semgrep)
            .map(|s| s.reference),
        GateMode::Strict | GateMode::Off => None,
    };

    let lockfiles: Vec<String> = super::git(root, &["ls-files"])?
        .lines()
        .filter(|f| {
            Path::new(f)
                .file_name()
                .is_some_and(|n| LOCKFILES.contains(&n.to_string_lossy().as_ref()))
        })
        // Central scope exclusion: excluded lockfiles are never scanned.
        .filter(|f| !super::scope::is_excluded(&config.gates.exclude, f))
        .map(str::to_owned)
        .collect();

    let results = std::thread::scope(|scope| {
        let leaks = scope.spawn(|| run_gitleaks(root, &gitleaks));
        let sg = scope.spawn(|| run_semgrep(root, &semgrep, &ruleset, semgrep_ref.as_deref()));
        let vulns = scope.spawn(|| run_osv(root, &osv, &lockfiles));
        vec![
            leaks.join().expect("gitleaks thread never panics"),
            sg.join().expect("semgrep thread never panics"),
            vulns.join().expect("osv thread never panics"),
        ]
    });

    let mut findings: Vec<Finding> = Vec::new();
    let mut tools_ran: Vec<&'static str> = Vec::new();
    for result in results {
        let mut part = result?;
        if let Some(note) = part.note {
            notes.push(note);
        }
        if part.ran {
            tools_ran.push(part.tool);
            findings.append(&mut part.findings);
        }
    }
    // Below-threshold findings inform but never block.
    let (enforceable, informational): (Vec<Finding>, Vec<Finding>) =
        findings.into_iter().partition(|f| f.severity >= threshold);
    if !informational.is_empty() {
        notes.push(format!(
            "security: {} finding(s) below the {threshold} threshold (informational)",
            informational.len()
        ));
    }
    let mut all = enforceable.clone();
    all.extend(informational);
    // The epilogue drops excluded findings from the enforceable set; the
    // full (visibility) set must not resurrect them.
    all.retain(|f| !super::scope::is_excluded(&config.gates.exclude, &f.file));

    let mut outcome = epilogue::finish(
        &epilogue::Epilogue {
            root,
            config,
            gate: "security",
            changed,
            mode,
        },
        enforceable,
        notes,
        tools_ran,
    )?;
    outcome.findings = all;
    Ok(outcome)
}

/// Record the security baseline: full scans, semgrep pinned to a commit
/// ref (HEAD), gitleaks + osv as snapshot fingerprints.
///
/// # Errors
/// Scanner or baseline-write failures.
pub fn record_baseline(root: &Path, config: &Config) -> Result<baseline::Baseline, GateError> {
    let outcome = run(root, config, None, GateMode::Strict)?;
    let threshold = config.gates.security_threshold.unwrap_or(Severity::High);
    let (semgrep_findings, snapshot): (Vec<Finding>, Vec<Finding>) = outcome
        .findings
        .iter()
        .filter(|f| f.severity >= threshold)
        .cloned()
        .partition(|f| f.tool == "semgrep");
    let head = super::git(root, &["rev-parse", "HEAD"])?.trim().to_owned();
    let mut base = baseline::Baseline::record("security", &snapshot);
    base.semgrep = Some(baseline::SemgrepBaseline {
        reference: head,
        count: semgrep_findings.len(),
    });
    baseline::save(root, &base)?;
    Ok(base)
}

/// One scanner's outcome: possibly skipped with a note.
struct ScanRun {
    tool: &'static str,
    ran: bool,
    findings: Vec<Finding>,
    note: Option<String>,
}

fn resolved(
    config: &Config,
    name: &str,
) -> Result<(&'static GateTool, tools::Resolved), GateError> {
    let tool = adapter::tool(name).expect("security tools are in the adapter table");
    let version = pinned(config, name);
    let resolved = tools::resolve(tool, &version)?;
    eprintln!("gate security: {} ({}) …", tool.name, resolved.via);
    Ok((tool, resolved))
}

fn pinned(config: &Config, name: &str) -> String {
    let tool = adapter::tool(name).expect("security tools are in the adapter table");
    config
        .gates
        .tools
        .get(name)
        .cloned()
        .unwrap_or_else(|| tool.default_version.to_owned())
}

/// gitleaks scans the whole git history (`git` mode); the JSON report goes
/// through a file because stdout carries logs.
fn run_gitleaks(
    root: &Path,
    (tool, resolved): &(&'static GateTool, tools::Resolved),
) -> Result<ScanRun, GateError> {
    let report_dir = root.join(".craftsman").join("cache");
    std::fs::create_dir_all(&report_dir).map_err(|source| GateError::Io {
        path: report_dir.clone(),
        source,
    })?;
    let report = report_dir.join("gitleaks-report.json");
    let mut argv = resolved.argv.clone();
    argv.extend(tool.base_args.iter().map(|s| (*s).to_owned()));
    argv.extend([
        "--report-format".to_owned(),
        "json".to_owned(),
        "--report-path".to_owned(),
        report.to_string_lossy().into_owned(),
        "--exit-code".to_owned(),
        "1".to_owned(),
        ".".to_owned(),
    ]);
    let output = exec(&argv, root, &[])?;
    let code = output.status.code().unwrap_or(-1);
    if !tool.success_codes.contains(&code) {
        return Err(GateError::ToolFailed {
            tool: "gitleaks".to_owned(),
            code: code.to_string(),
            output: tail(&String::from_utf8_lossy(&output.stderr), 30),
        });
    }
    let text = std::fs::read_to_string(&report).map_err(|source| GateError::Io {
        path: report.clone(),
        source,
    })?;
    let _ = std::fs::remove_file(&report);
    let findings = adapter::parse(tool, &text, "")?;
    Ok(ScanRun {
        tool: "gitleaks",
        ran: true,
        findings,
        note: None,
    })
}

/// semgrep over the pinned registry ruleset, offline, ERROR severity;
/// baseline mode adds `--baseline-commit` (its native diff-aware scan).
fn run_semgrep(
    root: &Path,
    (tool, resolved): &(&'static GateTool, tools::Resolved),
    ruleset: &Path,
    baseline_ref: Option<&str>,
) -> Result<ScanRun, GateError> {
    let mut argv = resolved.argv.clone();
    argv.extend(tool.base_args.iter().map(|s| (*s).to_owned()));
    argv.extend([
        "--config".to_owned(),
        ruleset.to_string_lossy().into_owned(),
        "--severity".to_owned(),
        "ERROR".to_owned(),
    ]);
    if let Some(reference) = baseline_ref {
        argv.push("--baseline-commit".to_owned());
        argv.push(reference.to_owned());
    }
    let note = baseline_ref.map(|reference| {
        format!(
            "semgrep: diff-aware against baseline commit {}",
            &reference[..reference.len().min(9)]
        )
    });
    let output = exec(&argv, root, &[])?;
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !tool.success_codes.contains(&code) {
        return Err(GateError::ToolFailed {
            tool: "semgrep".to_owned(),
            code: code.to_string(),
            output: tail(
                &format!("{stdout}{}", String::from_utf8_lossy(&output.stderr)),
                30,
            ),
        });
    }
    let findings = adapter::parse(tool, &stdout, "")?;
    Ok(ScanRun {
        tool: "semgrep",
        ran: true,
        findings,
        note,
    })
}

/// The osv-scanner database dir + argv: offline databases (downloaded on
/// first use — the one allowed network moment) and one `--lockfile` per
/// tracked lockfile.
fn osv_invocation(
    tool: &'static GateTool,
    resolved: &tools::Resolved,
    lockfiles: &[String],
) -> Result<(std::path::PathBuf, Vec<String>), GateError> {
    let db_dir = tools::tools_dir()?.join("osv-db");
    let first_use = !db_dir.is_dir();
    if first_use {
        std::fs::create_dir_all(&db_dir).map_err(|source| GateError::Io {
            path: db_dir.clone(),
            source,
        })?;
    }
    let mut argv = resolved.argv.clone();
    argv.extend(tool.base_args.iter().map(|s| (*s).to_owned()));
    argv.push("--offline-vulnerabilities".to_owned());
    if first_use {
        argv.push("--download-offline-databases".to_owned());
    }
    for lockfile in lockfiles {
        argv.push("--lockfile".to_owned());
        argv.push(lockfile.clone());
    }
    Ok((db_dir, argv))
}

/// osv-scanner over tracked lockfiles with local (offline) databases; the
/// first run downloads them, afterwards no network.
fn run_osv(
    root: &Path,
    (tool, resolved): &(&'static GateTool, tools::Resolved),
    lockfiles: &[String],
) -> Result<ScanRun, GateError> {
    if lockfiles.is_empty() {
        return Ok(ScanRun {
            tool: "osv-scanner",
            ran: false,
            findings: Vec::new(),
            note: Some("osv-scanner: no tracked lockfiles — skipped".to_owned()),
        });
    }
    let (db_dir, argv) = osv_invocation(tool, resolved, lockfiles)?;
    let output = exec(
        &argv,
        root,
        &[(
            "OSV_SCANNER_LOCAL_DB_CACHE_DIRECTORY",
            db_dir.to_string_lossy().into_owned(),
        )],
    )?;
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !tool.success_codes.contains(&code) {
        return Err(GateError::ToolFailed {
            tool: "osv-scanner".to_owned(),
            code: code.to_string(),
            output: tail(
                &format!("{stdout}{}", String::from_utf8_lossy(&output.stderr)),
                30,
            ),
        });
    }
    let mut findings = adapter::parse(tool, &stdout, "")?;
    // osv reports absolute lockfile paths; normalize to root-relative.
    let root_str = format!("{}/", root.display());
    for f in &mut findings {
        if let Some(stripped) = f.file.strip_prefix(&root_str) {
            f.file = stripped.to_owned();
        }
    }
    Ok(ScanRun {
        tool: "osv-scanner",
        ran: true,
        findings,
        note: None,
    })
}

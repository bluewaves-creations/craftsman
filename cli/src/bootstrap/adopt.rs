//! `craftsman adopt` — the brownfield five-phase state machine.
//!
//! Phase state lives in `.craftsman/adoption.toml` (CLI-written,
//! committed) so the craftsman-init adopt gear resumes across sessions.
//! Sequencing is enforced mechanically: a phase cannot start before its
//! predecessor's completion is recorded, and every transition records a
//! timestamp plus the git HEAD it happened at.
//!
//! The CLI owns mechanical phase actions only: Phase 1 writes the
//! gates-off craftsman.toml and the ADR-000 template; Phase 2 records a
//! baseline for every gate in baseline mode. Phases 0, 3, and 4 are
//! skill-driven — here they are state transitions and nothing else.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::templates;
use crate::config::Config;
use crate::gates::{self, GateError, baseline};

/// Committed phase-state file, root-relative.
pub const ADOPTION_REL: &str = ".craftsman/adoption.toml";

/// The five phases, by design (adopt.md): the ordering is the whole game.
pub const PHASES: [(u8, &str); 5] = [
    (0, "observe"),
    (1, "ledger"),
    (2, "hold-the-line"),
    (3, "recover"),
    (4, "steady-state"),
];

#[derive(Debug, Error)]
pub enum AdoptError {
    #[error(
        "no craftsman.toml and no git repository found from {start} — \
         adopt tracks state inside a repo; run `git init` first"
    )]
    NoRoot { start: PathBuf },
    #[error(
        "unknown phase {phase} — phases are 0..=4 (observe, ledger, hold-the-line, recover, steady-state)"
    )]
    UnknownPhase { phase: u8 },
    #[error(
        "cannot start phase {phase} — phase {blocker} is not complete yet \
         (the ordering is the whole game: ledger before gates, gates before \
         specs, specs before change); run `craftsman adopt --status`"
    )]
    OutOfOrder { phase: u8, blocker: u8 },
    #[error("phase {phase} is already {state} — run `craftsman adopt --status`")]
    AlreadyRecorded { phase: u8, state: &'static str },
    #[error("cannot complete phase {phase} — it was never started")]
    NotStarted { phase: u8 },
    #[error("invalid {ADOPTION_REL}: {detail}")]
    Corrupt { detail: String },
    #[error("cannot read or write {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Gate(#[from] GateError),
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
}

/// One phase's recorded transitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct PhaseRecord {
    pub phase: u8,
    pub started_at: String,
    pub started_head: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_head: Option<String>,
}

/// `.craftsman/adoption.toml` — the whole adoption state.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Adoption {
    #[serde(default = "version_one")]
    pub version: u32,
    #[serde(default)]
    pub phases: Vec<PhaseRecord>,
}

const fn version_one() -> u32 {
    1
}

impl Adoption {
    fn record(&self, phase: u8) -> Option<&PhaseRecord> {
        self.phases.iter().find(|p| p.phase == phase)
    }

    fn is_complete(&self, phase: u8) -> bool {
        self.record(phase).is_some_and(|p| p.completed_at.is_some())
    }

    /// The lowest phase not yet complete, `None` when all five are done.
    #[must_use]
    pub fn next_phase(&self) -> Option<u8> {
        PHASES
            .iter()
            .map(|(n, _)| *n)
            .find(|n| !self.is_complete(*n))
    }
}

/// The project root: nearest craftsman.toml ancestor, else the git
/// toplevel — adopt must run before the config exists (phases 0–1).
fn find_root(cwd: &Path) -> Result<PathBuf, AdoptError> {
    if let Ok(loaded) = Config::load(cwd) {
        return Ok(loaded.root);
    }
    gates::git(cwd, &["rev-parse", "--show-toplevel"]).map_or_else(
        |_| {
            Err(AdoptError::NoRoot {
                start: cwd.to_path_buf(),
            })
        },
        |out| Ok(PathBuf::from(out.trim())),
    )
}

fn load(root: &Path) -> Result<Adoption, AdoptError> {
    let path = root.join(ADOPTION_REL);
    if !path.is_file() {
        return Ok(Adoption::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|source| AdoptError::Io {
        path: path.clone(),
        source,
    })?;
    toml::from_str(&text).map_err(|e| AdoptError::Corrupt {
        detail: e.to_string(),
    })
}

fn save(root: &Path, adoption: &Adoption) -> Result<(), AdoptError> {
    let path = root.join(ADOPTION_REL);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| AdoptError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let text = toml::to_string_pretty(adoption).map_err(|e| AdoptError::Corrupt {
        detail: format!("cannot serialize: {e}"),
    })?;
    std::fs::write(&path, text).map_err(|source| AdoptError::Io { path, source })
}

/// Current HEAD, or `"none"` before the first commit — adoption state
/// must not require history that phase 0 is about to create.
fn head(root: &Path) -> String {
    gates::git(root, &["rev-parse", "HEAD"])
        .map_or_else(|_| "none".to_owned(), |o| o.trim().to_owned())
}

/// The status report (also returned by start/complete transitions).
#[derive(Debug, Serialize)]
pub struct Report {
    pub root: String,
    pub phases: Vec<PhaseRecord>,
    /// The lowest incomplete phase — where to resume.
    pub next_phase: Option<u8>,
    /// Mechanical actions the CLI performed during this transition.
    pub actions: Vec<String>,
}

/// `adopt --status`.
///
/// # Errors
/// Root/state errors only — an absent state file reports "not started".
pub fn status(cwd: &Path) -> Result<Report, AdoptError> {
    let root = find_root(cwd)?;
    let adoption = load(&root)?;
    Ok(Report {
        root: root.display().to_string(),
        next_phase: adoption.next_phase(),
        phases: adoption.phases,
        actions: Vec::new(),
    })
}

/// `adopt --start-phase N`.
///
/// # Errors
/// [`AdoptError::OutOfOrder`] when an earlier phase is incomplete;
/// [`AdoptError::AlreadyRecorded`] when N already started or completed.
pub fn start_phase(cwd: &Path, phase: u8) -> Result<Report, AdoptError> {
    let root = find_root(cwd)?;
    let mut adoption = load(&root)?;
    check_can_start(&adoption, phase)?;

    let mut actions = Vec::new();
    match phase {
        1 => phase_1_scaffold(&root, &mut actions)?,
        2 => phase_2_baselines(&root, &mut actions)?,
        _ => {}
    }

    adoption.phases.push(PhaseRecord {
        phase,
        started_at: baseline::iso_utc_now(),
        started_head: head(&root),
        completed_at: None,
        completed_head: None,
    });
    adoption.phases.sort_by_key(|p| p.phase);
    save(&root, &adoption)?;
    Ok(Report {
        root: root.display().to_string(),
        next_phase: adoption.next_phase(),
        phases: adoption.phases,
        actions,
    })
}

fn check_can_start(adoption: &Adoption, phase: u8) -> Result<(), AdoptError> {
    if !PHASES.iter().any(|(n, _)| *n == phase) {
        return Err(AdoptError::UnknownPhase { phase });
    }
    if let Some(record) = adoption.record(phase) {
        return Err(AdoptError::AlreadyRecorded {
            phase,
            state: if record.completed_at.is_some() {
                "complete"
            } else {
                "in progress"
            },
        });
    }
    if let Some(blocker) = (0..phase).find(|p| !adoption.is_complete(*p)) {
        return Err(AdoptError::OutOfOrder { phase, blocker });
    }
    Ok(())
}

/// `adopt --complete-phase N`.
///
/// # Errors
/// [`AdoptError::NotStarted`] / [`AdoptError::AlreadyRecorded`].
pub fn complete_phase(cwd: &Path, phase: u8) -> Result<Report, AdoptError> {
    let root = find_root(cwd)?;
    let mut adoption = load(&root)?;
    if !PHASES.iter().any(|(n, _)| *n == phase) {
        return Err(AdoptError::UnknownPhase { phase });
    }
    let head = head(&root);
    let Some(record) = adoption.phases.iter_mut().find(|p| p.phase == phase) else {
        return Err(AdoptError::NotStarted { phase });
    };
    if record.completed_at.is_some() {
        return Err(AdoptError::AlreadyRecorded {
            phase,
            state: "complete",
        });
    }
    record.completed_at = Some(baseline::iso_utc_now());
    record.completed_head = Some(head);
    save(&root, &adoption)?;
    Ok(Report {
        root: root.display().to_string(),
        next_phase: adoption.next_phase(),
        phases: adoption.phases,
        actions: Vec::new(),
    })
}

/// Phase 1 mechanical scaffold: gates-off craftsman.toml + ADR-000 —
/// written only where absent; adopt never overwrites project content.
fn phase_1_scaffold(root: &Path, actions: &mut Vec<String>) -> Result<(), AdoptError> {
    let config_path = root.join(crate::config::FILE_NAME);
    if config_path.is_file() {
        actions.push("craftsman.toml already exists — left untouched".to_owned());
    } else {
        let name = root.file_name().map_or_else(
            || "adopted-project".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        let text = templates::ADOPT_CONFIG_TOML
            .replace("__NAME__", &name)
            .replace("__VERSION__", env!("CARGO_PKG_VERSION"));
        write_new(&config_path, &text)?;
        actions.push("wrote craftsman.toml (all gates off; verify strict from birth)".to_owned());
    }

    let adr_path = root.join("decisions/ADR-000-adoption-baseline.md");
    if adr_path.is_file() {
        actions.push("decisions/ADR-000 already exists — left untouched".to_owned());
    } else {
        let now = baseline::iso_utc_now();
        let text = templates::ADR_000
            .replace("__DATE__", now.split('T').next().unwrap_or(&now))
            .replace("__HEAD__", &head(root));
        write_new(&adr_path, &text)?;
        actions.push("wrote decisions/ADR-000-adoption-baseline.md (template)".to_owned());
    }
    Ok(())
}

/// Phase 2 mechanical action: record a baseline for every gate currently
/// in baseline mode (the skill flips modes in craftsman.toml first).
fn phase_2_baselines(root: &Path, actions: &mut Vec<String>) -> Result<(), AdoptError> {
    let loaded = Config::load(root)?;
    let config = &loaded.config;
    let strict = crate::config::GateMode::Strict;
    let mut recorded = 0_usize;
    for gate in ["lint", "arch", "security", "health"] {
        if config.gates.mode(gate) != Some(crate::config::GateMode::Baseline) {
            continue;
        }
        let base = match gate {
            "lint" => record_snapshot(root, gate, &gates::lint::run(root, config, None, strict)?),
            "arch" => record_snapshot(root, gate, &gates::arch::run(root, config, None, strict)?),
            "health" => {
                record_snapshot(root, gate, &gates::health::run(root, config, None, strict)?)
            }
            _ => gates::security::record_baseline(root, config),
        }?;
        actions.push(format!(
            "gate {gate}: baseline recorded — {} finding(s) snapshotted",
            base.count()
        ));
        recorded += 1;
    }
    if recorded == 0 {
        actions.push(
            "no gate is in baseline mode — flip gates to \"baseline\" in \
             craftsman.toml, then record with `craftsman gate baseline <gate>`"
                .to_owned(),
        );
    }
    Ok(())
}

fn record_snapshot(
    root: &Path,
    gate: &str,
    outcome: &gates::GateOutcome,
) -> Result<baseline::Baseline, GateError> {
    let base = baseline::Baseline::record(gate, &outcome.findings);
    baseline::save(root, &base)?;
    Ok(base)
}

fn write_new(path: &Path, content: &str) -> Result<(), AdoptError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| AdoptError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, content).map_err(|source| AdoptError::Io {
        path: path.to_path_buf(),
        source,
    })
}

//! `craftsman check-all` — orchestrate the enabled gates.
//!
//! Order: verify → lint → security (Batch 6b gates slot in after),
//! honoring per-gate modes, with a file-hash cache that skips gates whose
//! inputs have not changed since their last green run.
//!
//! Cache: `.craftsman/cache/gates.json`, keyed per gate by a hash over
//! (gate, changed flag, HEAD when changed, craftsman.toml content, the
//! tracked-file set with working-tree modifications). Only green runs are
//! recorded; a cache hit is announced on stderr, never silent. Without a
//! usable git repo there is no cache — gates simply run.

use std::path::Path;

use serde_json::Value;

use super::{GateError, GateOutcome, changed_files, fnv_hex, git, lint, security};
use crate::config::{Config, GateMode};
use crate::verify::{self, Outcome as VerifyOutcome, Selection};

/// Cache file, root-relative (gitignored).
pub const CACHE_REL_PATH: &str = ".craftsman/cache/gates.json";

/// How one gate concluded within check-all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GateVerdict {
    Green,
    Red,
    /// Inputs unchanged since the last green run — skipped via the cache.
    CachedGreen,
    Off,
}

/// One gate's row in the summary.
#[derive(Debug, serde::Serialize)]
pub struct GateSummary {
    pub gate: &'static str,
    pub mode: String,
    pub verdict: GateVerdict,
    pub detail: String,
}

/// The check-all report: per-gate rows and the overall verdict.
#[derive(Debug)]
pub struct Report {
    pub gates: Vec<GateSummary>,
    /// Gate outcomes for gates that actually ran (findings for --json).
    pub outcomes: Vec<GateOutcome>,
}

impl Report {
    #[must_use]
    pub fn passed(&self) -> bool {
        self.gates.iter().all(|g| g.verdict != GateVerdict::Red)
    }
}

/// Run every enabled gate.
///
/// # Errors
/// [`GateError`] on tool/config failure — including a gate that is enabled
/// but not yet orchestrated (Batch 6b): enabled-but-unrunnable is exit 3,
/// never a silent skip.
pub fn run(root: &Path, config: &Config, changed: bool) -> Result<Report, GateError> {
    // Refuse silently-unrunnable configurations upfront.
    for (gate, mode) in config.gates.by_name() {
        if !matches!(gate, "verify" | "lint" | "security")
            && mode.is_some_and(|m| m != GateMode::Off)
        {
            return Err(GateError::UnsupportedGate {
                gate: gate.to_owned(),
            });
        }
    }

    let changed_set: Option<Vec<String>> = if changed {
        Some(changed_files(root)?)
    } else {
        None
    };

    let mut gates: Vec<GateSummary> = Vec::new();
    let mut outcomes: Vec<GateOutcome> = Vec::new();
    for (gate, mode) in config.gates.by_name() {
        if !matches!(gate, "verify" | "lint" | "security") {
            continue;
        }
        let Some(mode) = mode.filter(|m| *m != GateMode::Off) else {
            gates.push(GateSummary {
                gate,
                mode: "off".to_owned(),
                verdict: GateVerdict::Off,
                detail: "not enabled in craftsman.toml".to_owned(),
            });
            continue;
        };

        let key = cache_key(root, gate, changed).ok();
        if let Some(key) = &key
            && cache_lookup(root, gate).as_deref() == Some(key)
        {
            eprintln!("gate {gate}: inputs unchanged since last green run — skipped (cache hit)");
            gates.push(GateSummary {
                gate,
                mode: mode.to_string(),
                verdict: GateVerdict::CachedGreen,
                detail: "unchanged since last green run (cache)".to_owned(),
            });
            continue;
        }

        let summary = match gate {
            "verify" => run_verify(root, changed)?,
            "lint" => {
                let outcome = lint::run(root, config, changed_set.as_deref(), mode)?;
                let summary = summarize(gate, &outcome);
                outcomes.push(outcome);
                summary
            }
            "security" => {
                let outcome = security::run(root, config, changed_set.as_deref(), mode)?;
                let summary = summarize(gate, &outcome);
                outcomes.push(outcome);
                summary
            }
            _ => unreachable!("gate list is filtered above"),
        };
        let green = summary.verdict == GateVerdict::Green;
        gates.push(summary);
        if green {
            if let Some(key) = key {
                cache_store(root, gate, &key);
            }
        } else {
            // Fail fast: later gates are noise while an earlier one is red.
            break;
        }
    }

    Ok(Report { gates, outcomes })
}

fn summarize(gate: &'static str, outcome: &GateOutcome) -> GateSummary {
    for note in &outcome.notes {
        eprintln!("note: {note}");
    }
    if let Some(ratchet) = &outcome.ratchet {
        eprintln!("{ratchet}");
    }
    GateSummary {
        gate,
        mode: outcome.mode.to_string(),
        verdict: if outcome.passed() {
            GateVerdict::Green
        } else {
            GateVerdict::Red
        },
        detail: outcome.detail(),
    }
}

fn run_verify(root: &Path, changed: bool) -> Result<GateSummary, GateError> {
    eprintln!("gate verify: running the spec…");
    let selection = if changed {
        // --changed maps to the impact selection: scenarios the diff can
        // affect, falling back to everything loudly on a cold map.
        Selection::Impact("HEAD".to_owned())
    } else {
        Selection::All
    };
    let report = verify::run(root, &selection).map_err(Box::new)?;
    for w in &report.warnings {
        eprintln!("note: {w}");
    }
    let (verdict, detail) = match report.outcome {
        VerifyOutcome::Passed => (
            GateVerdict::Green,
            format!("{} scenarios green", report.counts.passed),
        ),
        VerifyOutcome::Failed => (
            GateVerdict::Red,
            format!(
                "{} failed, {} undefined, {} ambiguous",
                report.counts.failed, report.counts.undefined, report.counts.ambiguous
            ),
        ),
        // A filterless/impact run matching nothing: verify's own exit-4
        // contract belongs to `craftsman verify`; inside check-all an empty
        // spec is a red gate, not a crash.
        VerifyOutcome::EmptySelection => (
            GateVerdict::Red,
            "selection matched no scenarios".to_owned(),
        ),
    };
    Ok(GateSummary {
        gate: "verify",
        mode: "strict".to_owned(),
        verdict,
        detail,
    })
}

/// The cache key for a gate run: gate + changed flag (+ HEAD when changed)
/// + config content + tracked-file-set state.
///
/// Any git failure disables caching for the run (returned as Err and
/// swallowed by the caller — gates then simply run).
fn cache_key(root: &Path, gate: &str, changed: bool) -> Result<String, GateError> {
    let mut material = format!("{gate}\x1f{changed}\x1f");
    if changed {
        material.push_str(git(root, &["rev-parse", "HEAD"])?.trim());
    }
    let config_text =
        std::fs::read_to_string(root.join(crate::config::FILE_NAME)).map_err(|source| {
            GateError::Io {
                path: root.join(crate::config::FILE_NAME),
                source,
            }
        })?;
    material.push_str(&fnv_hex(&config_text));

    // Index state (blob ids cover committed + staged content) …
    material.push_str(&fnv_hex(&git(root, &["ls-files", "-s"])?));
    // … plus content hashes for anything the index does not pin: worktree
    // modifications and untracked files (NUL-separated: no quoting traps).
    let status = git(root, &["status", "--porcelain", "-z", "-uall"])?;
    let mut entries = status.split('\0').filter(|e| !e.is_empty());
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        let (xy, path) = entry.split_at(3);
        if xy.starts_with('R') || xy.starts_with('C') {
            let _ = entries.next(); // consume the rename source
        }
        material.push_str(path);
        let content = std::fs::read(root.join(path))
            .map_or_else(|_| "<gone>".to_owned(), |bytes| fnv_hex_bytes(&bytes));
        material.push_str(&content);
        material.push('\x1f');
    }
    Ok(fnv_hex(&material))
}

fn fnv_hex_bytes(bytes: &[u8]) -> String {
    format!("{:016x}", super::fnv64(bytes))
}

fn cache_lookup(root: &Path, gate: &str) -> Option<String> {
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(CACHE_REL_PATH)).ok()?).ok()?;
    doc[gate]["key"].as_str().map(str::to_owned)
}

/// Best-effort store — a failed cache write only costs a re-run.
fn cache_store(root: &Path, gate: &str, key: &str) {
    let path = root.join(CACHE_REL_PATH);
    let mut doc: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    doc[gate] = serde_json::json!({ "key": key, "at": super::baseline::iso_utc_now() });
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_ok()
    {
        let _ = std::fs::write(&path, format!("{doc:#}\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git_ok(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }

    #[test]
    fn cache_key_tracks_worktree_content() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::write(
            root.join("craftsman.toml"),
            "[project]\nname = \"x\"\nstacks = [\"rust\"]\n",
        )
        .expect("write config");
        std::fs::write(root.join("a.txt"), "one").expect("write");
        git_ok(root, &["init", "--quiet"]);
        git_ok(root, &["add", "-A"]);

        let k1 = cache_key(root, "lint", false).expect("key");
        assert_eq!(k1, cache_key(root, "lint", false).expect("key"), "stable");
        assert_ne!(
            k1,
            cache_key(root, "security", false).expect("key"),
            "gate name is part of the key"
        );

        std::fs::write(root.join("a.txt"), "two").expect("write");
        let k2 = cache_key(root, "lint", false).expect("key");
        assert_ne!(k1, k2, "unstaged content change must bust the cache");
    }

    #[test]
    fn cache_store_and_lookup_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(cache_lookup(tmp.path(), "lint"), None);
        cache_store(tmp.path(), "lint", "abc123");
        assert_eq!(cache_lookup(tmp.path(), "lint").as_deref(), Some("abc123"));
        assert_eq!(cache_lookup(tmp.path(), "security"), None);
    }
}

//! Setup operations: canonical install, per-agent fan-out, removal, and
//! status — every mutation attribution-checked (see the module doc).

use std::path::Path;

use include_dir::Dir;

use super::attest::{Attribution, attribute, points_into, sentinel_proves};
use super::{
    AGENTS, AgentSpec, Mode, Report, Row, SENTINEL, SetupError, canonical_dir, payload_digest,
    payload_files, payload_skills, skill_name,
};

/// Write an embedded skill dir to `dest` (no sentinel — see `install_copy`).
pub(super) fn extract(skill: &Dir<'static>, dest: &Path) -> Result<(), SetupError> {
    for (rel, bytes) in payload_files(skill) {
        let path = dest.join(&rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| SetupError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(&path, bytes).map_err(|source| SetupError::Io { path, source })?;
    }
    Ok(())
}

/// Extract + sentinel: the provenance-marked copy setup can later prove.
fn install_copy(skill: &Dir<'static>, dest: &Path, payload: &str) -> Result<(), SetupError> {
    if dest.exists() || dest.is_symlink() {
        remove_entry(dest)?;
    }
    extract(skill, dest)?;
    let sentinel = dest.join(SENTINEL);
    std::fs::write(
        &sentinel,
        format!("craftsman {}\n{payload}\n", env!("CARGO_PKG_VERSION")),
    )
    .map_err(|source| SetupError::Io {
        path: sentinel,
        source,
    })
}

fn remove_entry(path: &Path) -> Result<(), SetupError> {
    let result = if path.is_dir() && !path.is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    result.map_err(|source| SetupError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn row(scope: &str, skill: &str, action: &'static str, detail: String) -> Row {
    Row {
        scope: scope.to_owned(),
        skill: skill.to_owned(),
        action,
        detail,
    }
}

/// `craftsman setup` — canonical install, then agent fan-out.
///
/// # Errors
/// [`SetupError`] on IO/digest failure; attribution refusals are report
/// rows, never errors.
pub fn install(home: &Path, force: bool) -> Result<Report, SetupError> {
    let canonical = canonical_dir(home);
    std::fs::create_dir_all(&canonical).map_err(|source| SetupError::Io {
        path: canonical.clone(),
        source,
    })?;
    let mut rows = Vec::new();
    for skill in payload_skills()? {
        rows.push(canonical_skill(&canonical, skill, force)?);
    }
    for agent in detected_agents(home) {
        fan_out_agent(home, &canonical, agent, force, &mut rows)?;
    }
    Ok(Report {
        version: env!("CARGO_PKG_VERSION"),
        canonical_dir: canonical.display().to_string(),
        rows,
    })
}

fn canonical_skill(canonical: &Path, skill: &Dir<'static>, force: bool) -> Result<Row, SetupError> {
    let name = skill_name(skill);
    let dest = canonical.join(name);
    let payload = payload_digest(skill)?;
    let shown = dest.display().to_string();
    let (action, detail) = match attribute(&dest, &payload)? {
        Attribution::Absent => {
            install_copy(skill, &dest, &payload)?;
            ("installed", shown)
        }
        Attribution::Current => {
            if !sentinel_proves(&dest, &payload) {
                install_copy(skill, &dest, &payload)?; // refresh the sentinel
            }
            ("up-to-date", shown)
        }
        Attribution::OursStale => {
            install_copy(skill, &dest, &payload)?;
            ("updated", shown)
        }
        Attribution::SymlinkEntry if !force => (
            "left",
            format!("{shown} is a symlink you manage — --force replaces"),
        ),
        Attribution::FileEntry if !force => {
            ("left", format!("{shown} is a file — --force replaces"))
        }
        Attribution::Foreign if !force => (
            "left",
            format!("{shown} is not attributable to setup — --force replaces"),
        ),
        _ => {
            install_copy(skill, &dest, &payload)?;
            ("replaced", shown)
        }
    };
    Ok(row("canonical", name, action, detail))
}

fn detected_agents(home: &Path) -> impl Iterator<Item = &'static AgentSpec> {
    let home = home.to_path_buf();
    AGENTS.iter().filter(move |a| home.join(a.marker).is_dir())
}

fn fan_out_agent(
    home: &Path,
    canonical: &Path,
    agent: &AgentSpec,
    force: bool,
    rows: &mut Vec<Row>,
) -> Result<(), SetupError> {
    let skills_dir = home.join(agent.skills_subdir);
    if agent.mode == Mode::Standard {
        rows.push(row(
            agent.name,
            "*",
            "standard",
            format!("reads {} natively — nothing to link", canonical.display()),
        ));
        return Ok(());
    }
    if points_into(&skills_dir, canonical) {
        rows.push(row(
            agent.name,
            "*",
            "served",
            format!(
                "{} resolves into {} — the canonical install already serves this agent",
                skills_dir.display(),
                canonical.display()
            ),
        ));
        return Ok(());
    }
    std::fs::create_dir_all(&skills_dir).map_err(|source| SetupError::Io {
        path: skills_dir.clone(),
        source,
    })?;
    for skill in payload_skills()? {
        rows.push(link_skill(canonical, &skills_dir, skill, agent, force)?);
    }
    Ok(())
}

fn link_skill(
    canonical: &Path,
    skills_dir: &Path,
    skill: &Dir<'static>,
    agent: &AgentSpec,
    force: bool,
) -> Result<Row, SetupError> {
    let name = skill_name(skill);
    let target = skills_dir.join(name);
    let source = canonical.join(name);
    let shown = target.display().to_string();
    if target.is_symlink() {
        if points_into(&target, canonical) {
            return Ok(row(agent.name, name, "up-to-date", shown));
        }
        if !force {
            return Ok(row(
                agent.name,
                name,
                "left",
                format!("{shown} links elsewhere — --force replaces"),
            ));
        }
    } else if target.exists() {
        let payload = payload_digest(skill)?;
        match attribute(&target, &payload)? {
            Attribution::Current | Attribution::OursStale => {}
            _ if force => {}
            _ => {
                return Ok(row(
                    agent.name,
                    name,
                    "left",
                    format!("{shown} exists and is not attributable to setup — --force replaces"),
                ));
            }
        }
    }
    let replaced = target.exists() || target.is_symlink();
    if replaced {
        remove_entry(&target)?;
    }
    let action = if replaced { "replaced" } else { "linked" };
    #[cfg(unix)]
    if std::os::unix::fs::symlink(&source, &target).is_ok() {
        return Ok(row(agent.name, name, action, shown));
    }
    // Symlinks unavailable: provenance-marked copy instead.
    install_copy(skill, &target, &payload_digest(skill)?)?;
    Ok(row(
        agent.name,
        name,
        "copied",
        format!("{shown} (symlinks unavailable — re-run setup after upgrades)"),
    ))
}

/// `craftsman setup --remove` — mirror of install, same proofs.
///
/// # Errors
/// [`SetupError`] on IO/digest failure.
pub fn remove(home: &Path, force: bool) -> Result<Report, SetupError> {
    let canonical = canonical_dir(home);
    let mut rows = Vec::new();
    for agent in detected_agents(home) {
        remove_agent_links(home, &canonical, agent, &mut rows);
    }
    for skill in payload_skills()? {
        rows.push(remove_canonical_skill(&canonical, skill, force)?);
    }
    Ok(Report {
        version: env!("CARGO_PKG_VERSION"),
        canonical_dir: canonical.display().to_string(),
        rows,
    })
}

fn remove_agent_links(home: &Path, canonical: &Path, agent: &AgentSpec, rows: &mut Vec<Row>) {
    if agent.mode == Mode::Standard {
        return;
    }
    let skills_dir = home.join(agent.skills_subdir);
    if points_into(&skills_dir, canonical) || !skills_dir.is_dir() {
        return;
    }
    for skill in payload_skills().unwrap_or_default() {
        let name = skill_name(skill);
        let target = skills_dir.join(name);
        let shown = target.display().to_string();
        if target.is_symlink() && points_into(&target, canonical) {
            if remove_entry(&target).is_ok() {
                rows.push(row(agent.name, name, "removed", shown));
            }
        } else if target.is_dir() {
            let ours = payload_digest(skill).is_ok_and(|payload| {
                attribute(&target, &payload)
                    .is_ok_and(|a| matches!(a, Attribution::Current | Attribution::OursStale))
            });
            if ours && remove_entry(&target).is_ok() {
                rows.push(row(agent.name, name, "removed", shown));
            } else if !ours {
                rows.push(row(
                    agent.name,
                    name,
                    "left",
                    format!("{shown} is not attributable to setup — left in place"),
                ));
            }
        }
    }
}

fn remove_canonical_skill(
    canonical: &Path,
    skill: &Dir<'static>,
    force: bool,
) -> Result<Row, SetupError> {
    let name = skill_name(skill);
    let dest = canonical.join(name);
    let payload = payload_digest(skill)?;
    let shown = dest.display().to_string();
    let (action, detail) = match attribute(&dest, &payload)? {
        Attribution::Absent => ("absent", shown),
        Attribution::SymlinkEntry => ("left", format!("{shown} is a symlink you manage")),
        Attribution::Current | Attribution::OursStale => {
            remove_entry(&dest)?;
            ("removed", shown)
        }
        Attribution::FileEntry | Attribution::Foreign if force => {
            remove_entry(&dest)?;
            (
                "removed",
                format!("{shown} (forced — was not attributable)"),
            )
        }
        _ => (
            "left",
            format!("{shown} is not attributable to setup — --force removes"),
        ),
    };
    Ok(row("canonical", name, action, detail))
}

/// `craftsman setup --status` — what is installed where. Report-only.
///
/// # Errors
/// [`SetupError`] on digest failure.
pub fn status(home: &Path) -> Result<Report, SetupError> {
    let canonical = canonical_dir(home);
    let mut rows = Vec::new();
    for skill in payload_skills()? {
        let name = skill_name(skill);
        let dest = canonical.join(name);
        let payload = payload_digest(skill)?;
        let shown = dest.display().to_string();
        let (action, detail) = match attribute(&dest, &payload)? {
            Attribution::Absent => ("not-installed", "run `craftsman setup`".to_owned()),
            Attribution::Current => ("up-to-date", shown),
            Attribution::OursStale => ("stale", format!("{shown} — re-run `craftsman setup`")),
            Attribution::SymlinkEntry => ("symlink", format!("{shown} — managed by you")),
            _ => ("foreign", format!("{shown} — not attributable to setup")),
        };
        rows.push(row("canonical", name, action, detail));
    }
    for agent in detected_agents(home) {
        let skills_dir = home.join(agent.skills_subdir);
        let (action, detail) = match agent.mode {
            Mode::Standard => ("standard", "reads ~/.agents/skills natively".to_owned()),
            Mode::Link => {
                let linked = payload_skills()?
                    .iter()
                    .filter(|s| points_into(&skills_dir.join(skill_name(s)), &canonical))
                    .count();
                (
                    "linked",
                    format!("{linked}/6 skills served from {}", skills_dir.display()),
                )
            }
        };
        rows.push(row(agent.name, "*", action, detail));
    }
    Ok(Report {
        version: env!("CARGO_PKG_VERSION"),
        canonical_dir: canonical.display().to_string(),
        rows,
    })
}

//! Hermetic gate-tool resolution.
//!
//! Preference order (house rule, aligned with the uv/bun toolchain rule):
//! uvx/bunx runners wherever the tool ships on those registries — the
//! runner itself is hermetic (version pinned per invocation, zero install
//! state). Binary downloads into `~/.craftsman/tools/<tool>@<version>/`
//! only where neither registry applies (gitleaks, osv-scanner, swiftlint,
//! shellcheck). Toolchain tools (cargo fmt/clippy) resolve to the ambient
//! rust toolchain.
//!
//! Network happens only here, at install/first use. Once a tool is
//! resolved, the verdict path is offline. Downloaded artifacts get their
//! sha256 recorded in a local (never committed) manifest at
//! `~/.craftsman/tools/manifest.json`; a re-download that no longer matches
//! the recorded hash is refused.

use std::path::PathBuf;

use thiserror::Error;

use super::adapter::{GateTool, OsArch, Runner};

/// Errors resolving or installing a tool. Always exit code 3 — a missing or
/// broken tool is never a green gate.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("cannot determine a home directory for ~/.craftsman/tools")]
    NoHome,
    #[error("failed to run `{command}`")]
    Spawn {
        command: String,
        #[source]
        source: std::io::Error,
    },
    #[error("`{command}` failed (exit {code}):\n{output}")]
    CommandFailed {
        command: String,
        code: String,
        output: String,
    },
    #[error(
        "download of {url} produced sha256 {actual} but the local manifest \
         recorded {recorded} for {key} — refusing a changed artifact"
    )]
    ShaMismatch {
        url: String,
        key: String,
        actual: String,
        recorded: String,
    },
    #[error("cannot read or write {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "{tool}@{version}: no binary at {path} after install — the release \
         layout may have changed"
    )]
    BinaryMissing {
        tool: String,
        version: String,
        path: PathBuf,
    },
}

/// A resolved tool: the argv prefix to execute it with.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub argv: Vec<String>,
    /// Human note about how it resolved (for stderr progress).
    pub via: String,
}

/// `~/.craftsman/tools`, overridable for tests via `CRAFTSMAN_TOOLS_DIR`.
///
/// # Errors
/// [`ToolError::NoHome`] when no home directory can be determined.
pub fn tools_dir() -> Result<PathBuf, ToolError> {
    if let Ok(dir) = std::env::var("CRAFTSMAN_TOOLS_DIR") {
        return Ok(PathBuf::from(dir));
    }
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".craftsman").join("tools"))
        .ok_or(ToolError::NoHome)
}

/// Resolve a tool at its pinned version, installing on first use where the
/// runner needs an install step.
///
/// # Errors
/// [`ToolError`] when the tool cannot be resolved — never a silent skip.
pub fn resolve(tool: &GateTool, version: &str) -> Result<Resolved, ToolError> {
    match &tool.runner {
        Runner::Toolchain { program } => Ok(Resolved {
            argv: vec![(*program).to_owned()],
            via: format!("{program} (ambient toolchain)"),
        }),
        Runner::Uvx => Ok(Resolved {
            argv: vec!["uvx".to_owned(), format!("{}@{version}", tool.name)],
            via: format!("uvx {}@{version}", tool.name),
        }),
        Runner::Bunx { package } => Ok(Resolved {
            argv: vec!["bunx".to_owned(), format!("{package}@{version}")],
            via: format!("bunx {package}@{version}"),
        }),
        Runner::Github {
            repo,
            asset,
            archive,
            binary,
            os_arch,
            path_fallback,
        } => resolve_github(
            tool.name,
            version,
            repo,
            asset,
            *archive,
            binary,
            *os_arch,
            *path_fallback,
        ),
    }
}

/// os/arch tokens for the current build target (tools run on this machine).
const fn tokens(style: OsArch) -> (&'static str, &'static str) {
    match style {
        OsArch::Go => (
            if cfg!(target_os = "macos") {
                "darwin"
            } else {
                "linux"
            },
            if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "amd64"
            },
        ),
        OsArch::GoX64 => (
            if cfg!(target_os = "macos") {
                "darwin"
            } else {
                "linux"
            },
            if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "x64"
            },
        ),
        OsArch::Uname => (
            if cfg!(target_os = "macos") {
                "darwin"
            } else {
                "linux"
            },
            if cfg!(target_arch = "aarch64") {
                "aarch64"
            } else {
                "x86_64"
            },
        ),
        OsArch::K6 => (
            if cfg!(target_os = "macos") {
                "macos"
            } else {
                "linux"
            },
            if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "amd64"
            },
        ),
        OsArch::None => ("", ""),
    }
}

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "the {version}/{os}/{arch} placeholders are adapter-table data, not format args"
)]
pub(super) fn fill(template: &str, version: &str, os: &str, arch: &str) -> String {
    template
        .replace("{version}", version)
        .replace("{os}", os)
        .replace("{arch}", arch)
}

#[expect(
    clippy::too_many_arguments,
    reason = "private plumbing directly mirroring the Runner::Github fields"
)]
mod install;

pub use install::ensure_download;
use install::resolve_github;
pub(crate) use install::sha256;

/// Offline resolution status of `[gates.tools]` pins — doctor's
/// gate-tools check.
///
/// Returns `(resolved, missing)` human descriptions: hermetic release
/// binaries must already sit under [`tools_dir`], registry runners need
/// their runner on PATH, toolchain tools probe the ambient program.
/// Reports only; never installs.
#[must_use]
pub fn pin_status(pins: &std::collections::BTreeMap<String, String>) -> (Vec<String>, Vec<String>) {
    use super::adapter::{Runner, tool};
    let probe = |program: &str| {
        std::process::Command::new(program)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    };
    let (mut resolved, mut missing) = (Vec::new(), Vec::new());
    for (name, version) in pins {
        let pin = format!("{name}@{version}");
        match tool(name).map(|t| &t.runner) {
            None => missing.push(format!("{pin} (not a known gate tool)")),
            Some(Runner::Toolchain { program }) => {
                if probe(program) {
                    resolved.push(pin);
                } else {
                    missing.push(format!("{pin} (toolchain program `{program}` not found)"));
                }
            }
            Some(Runner::Uvx) => {
                if probe("uv") {
                    resolved.push(pin);
                } else {
                    missing.push(format!("{pin} (runs via uvx — install uv)"));
                }
            }
            Some(Runner::Bunx { .. }) => {
                if probe("bun") {
                    resolved.push(pin);
                } else {
                    missing.push(format!("{pin} (runs via bunx — install bun)"));
                }
            }
            Some(Runner::Github { .. }) => match tools_dir() {
                Ok(dir) if dir.join(&pin).is_dir() => resolved.push(pin),
                Ok(dir) => missing.push(format!(
                    "{pin} (no hermetic install under {} — the first run of its \
                     gate downloads it; run that gate once while online)",
                    dir.display()
                )),
                Err(err) => missing.push(format!("{pin} ({err})")),
            },
        }
    }
    (resolved, missing)
}

#[cfg(test)]
mod tests {
    use super::install::version_tag;
    use super::*;
    use crate::gates::adapter;

    #[test]
    fn registry_runners_resolve_without_touching_disk() {
        let ruff = adapter::tool("ruff").expect("ruff in table");
        let r = resolve(ruff, "0.13.2").expect("uvx resolution is pure");
        assert_eq!(r.argv, vec!["uvx".to_owned(), "ruff@0.13.2".to_owned()]);

        let biome = adapter::tool("biome").expect("biome in table");
        let r = resolve(biome, "2.2.5").expect("bunx resolution is pure");
        assert_eq!(
            r.argv,
            vec!["bunx".to_owned(), "@biomejs/biome@2.2.5".to_owned()]
        );

        let clippy = adapter::tool("clippy").expect("clippy in table");
        let r = resolve(clippy, "toolchain").expect("toolchain resolution is pure");
        assert_eq!(r.argv, vec!["cargo".to_owned()]);
    }

    #[test]
    fn asset_templates_fill_version_os_arch() {
        assert_eq!(
            fill(
                "gitleaks_{version}_{os}_{arch}.tar.gz",
                "8.24.0",
                "darwin",
                "arm64"
            ),
            "gitleaks_8.24.0_darwin_arm64.tar.gz"
        );
        assert_eq!(version_tag("gitleaks/gitleaks", "8.24.0"), "v8.24.0");
        assert_eq!(version_tag("realm/SwiftLint", "0.57.0"), "0.57.0");
    }
}

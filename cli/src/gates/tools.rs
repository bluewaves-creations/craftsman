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

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use thiserror::Error;

use super::adapter::{Archive, GateTool, OsArch, Runner};

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
        OsArch::None => ("", ""),
    }
}

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "the {version}/{os}/{arch} placeholders are adapter-table data, not format args"
)]
fn fill(template: &str, version: &str, os: &str, arch: &str) -> String {
    template
        .replace("{version}", version)
        .replace("{os}", os)
        .replace("{arch}", arch)
}

#[expect(
    clippy::too_many_arguments,
    reason = "private plumbing directly mirroring the Runner::Github fields"
)]
fn resolve_github(
    name: &str,
    version: &str,
    repo: &str,
    asset: &str,
    archive: Archive,
    binary: &str,
    os_arch: OsArch,
    path_fallback: bool,
) -> Result<Resolved, ToolError> {
    let (os, arch) = tokens(os_arch);
    let dir = tools_dir()?.join(format!("{name}@{version}"));
    let bin = dir.join(fill(binary, version, os, arch));
    if bin.is_file() {
        return Ok(Resolved {
            argv: vec![bin.to_string_lossy().into_owned()],
            via: format!("{} (hermetic)", bin.display()),
        });
    }

    let asset_name = fill(asset, version, os, arch);
    let url = format!(
        "https://github.com/{repo}/releases/download/{version_tag}/{asset_name}",
        version_tag = version_tag(repo, version)
    );
    match install(name, version, &dir, &bin, &url, archive) {
        Ok(()) => Ok(Resolved {
            argv: vec![bin.to_string_lossy().into_owned()],
            via: format!("{} (installed from {url})", bin.display()),
        }),
        Err(err) if path_fallback => {
            // Ambient fallback (e.g. brew shellcheck): unpinned, but honest
            // about it — and only after the hermetic path failed.
            if let Some(ambient) = ambient(name) {
                eprintln!(
                    "tool {name}: hermetic install failed ({err}); \
                     falling back to ambient {ambient} (version unpinned)"
                );
                return Ok(Resolved {
                    argv: vec![name.to_owned()],
                    via: format!("{ambient} (ambient fallback)"),
                });
            }
            Err(err)
        }
        Err(err) => Err(err),
    }
}

/// `SwiftLint` tags releases without a `v` prefix; the Go tools use `v`.
fn version_tag(repo: &str, version: &str) -> String {
    if repo == "realm/SwiftLint" {
        version.to_owned()
    } else {
        format!("v{version}")
    }
}

/// An ambient binary of this name on PATH, if any.
fn ambient(name: &str) -> Option<String> {
    let ok = Command::new(name)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    ok.then(|| name.to_owned())
}

/// Download + extract + record sha256. The one network moment.
fn install(
    name: &str,
    version: &str,
    dir: &Path,
    bin: &Path,
    url: &str,
    archive: Archive,
) -> Result<(), ToolError> {
    eprintln!("tool {name}@{version}: installing from {url} …");
    std::fs::create_dir_all(dir).map_err(|source| ToolError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    let download = dir.join(".download");
    run(
        "curl",
        &[
            "-fsSL",
            "--retry",
            "2",
            "-o",
            &download.to_string_lossy(),
            url,
        ],
        dir,
    )?;
    let sha = sha256(&download);
    record_sha(name, version, url, &sha)?;

    let dl = download.to_string_lossy().into_owned();
    let target = dir.to_string_lossy().into_owned();
    match archive {
        Archive::TarGz => run("tar", &["-xzf", &dl, "-C", &target], dir)?,
        Archive::TarXz => run("tar", &["-xJf", &dl, "-C", &target], dir)?,
        Archive::Zip => run("unzip", &["-oq", &dl, "-d", &target], dir)?,
        Archive::Raw => std::fs::rename(&download, bin).map_err(|source| ToolError::Io {
            path: bin.to_path_buf(),
            source,
        })?,
    }
    if archive != Archive::Raw {
        let _ = std::fs::remove_file(&download);
    }
    if !bin.is_file() {
        return Err(ToolError::BinaryMissing {
            tool: name.to_owned(),
            version: version.to_owned(),
            path: bin.to_path_buf(),
        });
    }
    // chmod +x (tar/zip may or may not preserve the bit; raw never has it).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(bin, perms).map_err(|source| ToolError::Io {
            path: bin.to_path_buf(),
            source,
        })?;
    }
    eprintln!("tool {name}@{version}: installed (sha256 {sha})");
    Ok(())
}

/// Fetch a URL to a file under the tools dir if not already present
/// (semgrep's pinned ruleset uses this). Returns the local path.
///
/// # Errors
/// [`ToolError`] on download failure when the file is absent.
pub fn ensure_download(key: &str, file_name: &str, url: &str) -> Result<PathBuf, ToolError> {
    let dir = tools_dir()?.join(key);
    let path = dir.join(file_name);
    if path.is_file() {
        return Ok(path);
    }
    eprintln!("tool {key}: fetching {url} …");
    std::fs::create_dir_all(&dir).map_err(|source| ToolError::Io {
        path: dir.clone(),
        source,
    })?;
    let tmp = dir.join(".download");
    run(
        "curl",
        &["-fsSL", "--retry", "2", "-o", &tmp.to_string_lossy(), url],
        &dir,
    )?;
    let sha = sha256(&tmp);
    record_sha(key, file_name, url, &sha)?;
    std::fs::rename(&tmp, &path).map_err(|source| ToolError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

fn run(program: &str, args: &[&str], dir: &Path) -> Result<(), ToolError> {
    let command = format!("{program} {}", args.join(" "));
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|source| ToolError::Spawn {
            command: command.clone(),
            source,
        })?;
    if !output.status.success() {
        return Err(ToolError::CommandFailed {
            command,
            code: output
                .status
                .code()
                .map_or_else(|| "signal".to_owned(), |c| c.to_string()),
            output: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    Ok(())
}

/// sha256 via the system `shasum -a 256` (macOS) or `sha256sum` (linux) —
/// no crypto dependency for a local integrity note. `"unavailable"` when
/// neither exists (the manifest then records that honestly).
fn sha256(path: &Path) -> String {
    for (program, args) in [("shasum", &["-a", "256"][..]), ("sha256sum", &[][..])] {
        if let Ok(o) = Command::new(program).args(args).arg(path).output()
            && o.status.success()
            && let Some(hash) = String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .next()
                .map(str::to_owned)
        {
            return hash;
        }
    }
    "unavailable".to_owned()
}

/// Record (or verify) the artifact hash in the local manifest. First
/// install records; a later download must match or resolution fails.
fn record_sha(name: &str, version: &str, url: &str, sha: &str) -> Result<(), ToolError> {
    let manifest_path = tools_dir()?.join("manifest.json");
    let mut doc: Value = std::fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    let key = format!("{name}@{version}");
    if let Some(recorded) = doc[&key]["sha256"].as_str()
        && recorded != "unavailable"
        && sha != "unavailable"
        && recorded != sha
    {
        return Err(ToolError::ShaMismatch {
            url: url.to_owned(),
            key,
            actual: sha.to_owned(),
            recorded: recorded.to_owned(),
        });
    }
    doc[&key] = serde_json::json!({ "sha256": sha, "url": url });
    if let Some(parent) = manifest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ToolError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(&manifest_path, format!("{doc:#}\n")).map_err(|source| ToolError::Io {
        path: manifest_path,
        source,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
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

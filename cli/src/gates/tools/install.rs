//! Hermetic tool installation: GitHub release download, archive
//! extraction, sha256 recording, and one-off file downloads — the write
//! side of the resolution in the parent module.

use std::path::{Path, PathBuf};

use std::process::Command;

use serde_json::Value;

use super::super::adapter::{Archive, OsArch};
use super::{Resolved, ToolError, fill, tokens, tools_dir};

pub(super) fn resolve_github(
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
pub(super) fn version_tag(repo: &str, version: &str) -> String {
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

/// sha256 via the system `shasum -a 256` (macOS) or `sha256sum` (linux).
///
/// No crypto dependency for a local integrity note. `"unavailable"` when
/// neither exists (the manifest then records that honestly). Also used by
/// the docs cache manifest (Batch 7).
#[must_use]
pub fn sha256(path: &Path) -> String {
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

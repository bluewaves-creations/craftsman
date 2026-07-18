//! Lockfile version resolution for `docs status` and cache keying.
//!
//! Each candidate directory (project root + every configured
//! `[verify.*] cwd`) may hold Cargo.lock, uv.lock, bun.lock, or
//! Package.resolved; the first lockfile naming the dependency wins.
//! These parses feed staleness advice and cache keys, never a verdict.

use std::path::{Path, PathBuf};

/// The lockfiles `docs status`/version resolution consult: each candidate
/// directory (project root + every configured `[verify.*] cwd`) may hold
/// Cargo.lock, uv.lock, bun.lock, or Package.resolved.
#[must_use]
pub fn lockfile_paths(root: &Path, config: &crate::config::Config) -> Vec<PathBuf> {
    let mut dirs = vec![root.to_path_buf()];
    for stack in ["rust", "python", "typescript", "swift", "bash"] {
        if let Some(cwd) = config.verify.stack(stack).and_then(|s| s.cwd.as_deref()) {
            dirs.push(root.join(cwd));
        }
    }
    let mut found = Vec::new();
    for dir in dirs {
        for name in ["Cargo.lock", "uv.lock", "bun.lock", "Package.resolved"] {
            let p = dir.join(name);
            if p.is_file() && !found.contains(&p) {
                found.push(p);
            }
        }
    }
    found
}

/// The pinned version of `name` in one lockfile, if listed.
///
/// Cargo.lock and uv.lock are TOML `[[package]] name/version` arrays;
/// bun.lock and Package.resolved are scanned textually (bun.lock is JSONC,
/// Package.resolved nests `identity`/`version` pairs) — a version note for
/// staleness advice, not a verdict input.
#[must_use]
pub fn lockfile_version(lockfile: &Path, text: &str, name: &str) -> Option<String> {
    match lockfile.file_name().and_then(|n| n.to_str()) {
        Some("Cargo.lock" | "uv.lock") => toml_package_version(text, name),
        Some("bun.lock") => bun_lock_version(text, name),
        Some("Package.resolved") => package_resolved_version(text, name),
        _ => None,
    }
}

/// `[[package]] name = "…" / version = "…"` scan, tolerant of key order.
fn toml_package_version(text: &str, name: &str) -> Option<String> {
    let mut in_matching = false;
    let mut version: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("[[package]]") || line.starts_with('[') {
            if in_matching && version.is_some() {
                return version;
            }
            in_matching = false;
            version = None;
            continue;
        }
        if let Some(v) = quoted_value(line, "name")
            && v == name
        {
            in_matching = true;
        }
        if let Some(v) = quoted_value(line, "version") {
            version = Some(v);
        }
        if in_matching && version.is_some() && line.is_empty() {
            return version;
        }
    }
    in_matching.then_some(version).flatten()
}

/// `key = "value"` on one TOML line.
fn quoted_value(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start().strip_prefix('=')?;
    let rest = rest.trim();
    let inner = rest.strip_prefix('"')?;
    inner.split('"').next().map(str::to_owned)
}

/// bun.lock pins appear as `"name@version"` strings.
fn bun_lock_version(text: &str, name: &str) -> Option<String> {
    let needle = format!("\"{name}@");
    let at = text.find(&needle)?;
    let rest = &text[at + needle.len()..];
    let version: String = rest
        .chars()
        .take_while(|c| *c != '"' && *c != ',')
        .collect();
    (!version.is_empty() && version.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .then_some(version)
}

/// Package.resolved: `"identity" : "name"` followed (within the same pin
/// object) by `"version" : "x.y.z"`.
fn package_resolved_version(text: &str, name: &str) -> Option<String> {
    let needle = format!("\"identity\" : \"{name}\"");
    let alt = format!("\"identity\": \"{name}\"");
    let at = text.find(&needle).or_else(|| text.find(&alt))?;
    let rest = &text[at..];
    let vkey = rest.find("\"version\"")?;
    let after = &rest[vkey + "\"version\"".len()..];
    let open = after.find('"')?;
    let value = &after[open + 1..];
    value.split('"').next().map(str::to_owned)
}

/// The version of `name` across every lockfile present, first hit wins.
#[must_use]
pub fn resolve_lockfile_version(
    root: &Path,
    config: &crate::config::Config,
    name: &str,
) -> Option<String> {
    for lockfile in lockfile_paths(root, config) {
        if let Ok(text) = std::fs::read_to_string(&lockfile)
            && let Some(v) = lockfile_version(&lockfile, &text, name)
        {
            return Some(v);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // Constructed sample mirroring real Cargo.lock 2026 layout (key order
    // observed in this repo's cli/Cargo.lock).
    const CARGO_LOCK: &str = r#"
version = 4

[[package]]
name = "clap"
version = "4.6.2"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "gherkin"
version = "0.16.0"
"#;

    #[test]
    fn cargo_lock_versions_resolve() {
        let p = Path::new("Cargo.lock");
        assert_eq!(
            lockfile_version(p, CARGO_LOCK, "clap").as_deref(),
            Some("4.6.2")
        );
        assert_eq!(
            lockfile_version(p, CARGO_LOCK, "gherkin").as_deref(),
            Some("0.16.0")
        );
        assert_eq!(lockfile_version(p, CARGO_LOCK, "tokio"), None);
    }

    #[test]
    fn bun_lock_versions_resolve() {
        // Constructed from the bun.lock JSONC shape in
        // cli/tests/fixtures/ts-todo/bun.lock (bun 1.3.14).
        let text = r#"{
  "packages": {
    "hono": ["hono@4.6.14", "", {}, "sha512-x"],
  }
}"#;
        let p = Path::new("bun.lock");
        assert_eq!(lockfile_version(p, text, "hono").as_deref(), Some("4.6.14"));
        assert_eq!(lockfile_version(p, text, "react"), None);
    }

    #[test]
    fn package_resolved_versions_resolve() {
        // Constructed per Package.resolved v2 (observed in the S1 spike).
        let text = r#"{
  "pins" : [
    {
      "identity" : "swift-nio",
      "kind" : "remoteSourceControl",
      "state" : {
        "revision" : "abc",
        "version" : "2.81.0"
      }
    }
  ],
  "version" : 2
}"#;
        let p = Path::new("Package.resolved");
        assert_eq!(
            lockfile_version(p, text, "swift-nio").as_deref(),
            Some("2.81.0")
        );
    }
}

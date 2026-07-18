//! `craftsman arch` — dependency-direction fitness rules v1.
//!
//! Research verdict (production-grade doc §arch): architecture rules
//! belong in fitness functions, not prose — a dependency-direction check
//! that fails the build is followed 100% of the time; prose is not. No
//! incumbent rule engine exists for Rust/Swift (design doc open item #3),
//! so this is craftsman's own, deliberately small v1.
//!
//! Rules live in `[arch] deny = ["A -> B", …]` where A and B are path
//! prefixes relative to the stack root: a file under A that imports
//! anything resolving under B is a violation. Scope correction vs the
//! design-doc sketch (ADR-004): arch is dependency direction ONLY —
//! `max-file-lines` is a health metric and lives in `[health]`.
//!
//! Import extraction is textual per stack (no full parsers in v1):
//!
//! - **rust**: `use crate::a::b…` (incl. `pub use`, single-level `{…}`
//!   groups, multi-line statements joined until `;`) → `src/a/b…`.
//!   `super::`/`self::` imports are not resolved (crate-absolute paths are
//!   how cross-module dependencies are written; documented limit).
//! - **python**: `import a.b`, `from a.b import c` → `a/b/c`; relative
//!   `from .x import y` resolves against the importing file's directory.
//! - **typescript**: `import … from './x'`, `export … from`, `require()`
//!   — only relative specifiers resolve (package imports are external;
//!   tsconfig path aliases are not resolved — documented limit).
//! - **swift**: `import Module` only — module-level granularity, mapped to
//!   a source directory via the target names in `Package.swift`
//!   (`path:` when given, else `Sources/<name>` / `Tests/<name>`).
//!   Coarser than file-level imports by nature; documented.
//! - **bash**: `source path` / `. path` relative to the sourcing file
//!   (paths containing `$` expansions are skipped — documented limit).
//!
//! Prefix matching is extension-blind: `src/a.rs` counts as under prefix
//! `src/a` (single-file Rust/Python modules are the same boundary as
//! their directory form). The scan always covers the whole graph —
//! `--changed` cannot narrow a structural property.

use std::collections::BTreeMap;
use std::path::Path;

use super::{Finding, GateError, GateOutcome, Severity, epilogue};
use crate::config::{Config, GateMode};

/// The gate/tool name for findings and baselines.
const TOOL: &str = "arch";

/// One parsed `"A -> B"` rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub from: String,
    pub to: String,
}

/// Run the arch gate.
///
/// # Errors
/// [`GateError::NotConfigured`] when `[arch] deny` is absent or empty (an
/// enabled gate with zero rules would be silent green);
/// [`GateError::BadArchRule`] on a malformed rule; census/read failures.
pub fn run(
    root: &Path,
    config: &Config,
    changed: Option<&[String]>,
    mode: GateMode,
) -> Result<GateOutcome, GateError> {
    let rules = parse_rules(&config.arch.deny)?;
    let mut notes: Vec<String> = Vec::new();
    if changed.is_some() {
        notes.push(
            "arch: --changed never narrows this gate (dependency direction \
             is a whole-graph property) — running in full"
                .to_owned(),
        );
    }

    let mut findings: Vec<Finding> = Vec::new();
    let tracked = super::git(root, &["ls-files"])?;
    for stack in &config.project.stacks {
        let Some(lang) = StackLang::for_stack(stack) else {
            notes.push(format!("arch: stack {stack} has no import extractor"));
            continue;
        };
        let cwd = config
            .verify
            .stack(stack)
            .and_then(|s| s.cwd.as_deref())
            .map(|c| c.trim_end_matches('/').to_owned());
        let prefix = cwd.as_deref().map(|c| format!("{c}/"));
        let swift_targets = if lang == StackLang::Swift {
            swift_target_map(root, cwd.as_deref())
        } else {
            BTreeMap::new()
        };
        if lang == StackLang::Swift && swift_targets.is_empty() {
            notes.push(
                "arch: no Package.swift targets found — swift imports \
                 cannot be mapped to paths (module-level granularity needs \
                 the target table)"
                    .to_owned(),
            );
            continue;
        }
        for file in tracked.lines() {
            // Stack-root-relative path of a file belonging to this stack.
            let rel = match &prefix {
                Some(p) => match file.strip_prefix(p) {
                    Some(r) => r,
                    None => continue,
                },
                None => file,
            };
            if !lang.owns(rel) {
                continue;
            }
            let text =
                std::fs::read_to_string(root.join(file)).map_err(|source| GateError::Io {
                    path: root.join(file),
                    source,
                })?;
            for (line, target) in extract_imports(lang, rel, &text, &swift_targets) {
                for rule in &rules {
                    if under(rel, &rule.from) && under(&target, &rule.to) {
                        findings.push(Finding {
                            gate: "arch",
                            tool: TOOL,
                            rule: "denied-dependency".to_owned(),
                            file: file.to_owned(),
                            line: Some(line),
                            message: format!(
                                "{rel} imports `{target}` — denied by arch rule \
                                 \"{} -> {}\"",
                                rule.from, rule.to
                            ),
                            severity: Severity::High,
                        });
                    }
                }
            }
        }
    }
    findings.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    epilogue::finish(
        &epilogue::Epilogue {
            root,
            config,
            gate: "arch",
            changed,
            mode,
        },
        findings,
        notes,
        vec![TOOL],
    )
}

/// Parse `[arch] deny` into rules, refusing empty or malformed input.
///
/// # Errors
/// See [`run`].
pub fn parse_rules(deny: &[String]) -> Result<Vec<Rule>, GateError> {
    if deny.is_empty() {
        return Err(GateError::NotConfigured {
            gate: "arch",
            hint: "add [arch] deny = [\"A -> B\", …] to craftsman.toml \
                   (path prefixes relative to the stack root)"
                .to_owned(),
        });
    }
    deny.iter()
        .map(|rule| {
            let (from, to) = rule
                .split_once("->")
                .ok_or_else(|| GateError::BadArchRule { rule: rule.clone() })?;
            let (from, to) = (from.trim(), to.trim());
            if from.is_empty() || to.is_empty() {
                return Err(GateError::BadArchRule { rule: rule.clone() });
            }
            Ok(Rule {
                from: from.trim_matches('/').to_owned(),
                to: to.trim_matches('/').to_owned(),
            })
        })
        .collect()
}

/// Is `path` (stack-root-relative) under `prefix`? Extension-blind:
/// `src/a.rs` is under `src/a`.
fn under(path: &str, prefix: &str) -> bool {
    if path == prefix || path.starts_with(&format!("{prefix}/")) {
        return true;
    }
    stem(path) == prefix
}

/// `path` without its final extension (`src/a.rs` → `src/a`).
fn stem(path: &str) -> &str {
    let file_start = path.rfind('/').map_or(0, |i| i + 1);
    path[file_start..]
        .rfind('.')
        .map_or(path, |dot| &path[..file_start + dot])
}

/// Languages the extractors understand, keyed by stack name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StackLang {
    Rust,
    Python,
    Ts,
    Swift,
    Bash,
}

impl StackLang {
    fn for_stack(stack: &str) -> Option<Self> {
        match stack {
            "rust" => Some(Self::Rust),
            "python" => Some(Self::Python),
            "typescript" => Some(Self::Ts),
            "swift" | "swift-apple" => Some(Self::Swift),
            "bash" => Some(Self::Bash),
            _ => None,
        }
    }

    /// Does this stack own `path` (by extension)?
    fn owns(self, path: &str) -> bool {
        let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) else {
            return false;
        };
        match self {
            Self::Rust => ext == "rs",
            Self::Python => ext == "py",
            Self::Ts => matches!(ext, "ts" | "tsx" | "js" | "jsx"),
            Self::Swift => ext == "swift",
            Self::Bash => matches!(ext, "sh" | "bash"),
        }
    }
}

/// Extract `(line, stack-root-relative target)` imports from one file.
fn extract_imports(
    lang: StackLang,
    rel_path: &str,
    text: &str,
    swift_targets: &BTreeMap<String, String>,
) -> Vec<(u64, String)> {
    match lang {
        StackLang::Rust => rust_imports(text),
        StackLang::Python => python_imports(rel_path, text),
        StackLang::Ts => ts_imports(rel_path, text),
        StackLang::Swift => swift_imports(text, swift_targets),
        StackLang::Bash => bash_imports(rel_path, text),
    }
}

/// `use crate::a::b::{c, d as e};` → `src/a/b/c`, `src/a/b/d`. Multi-line
/// statements are joined until `;`; nested brace groups are not expanded
/// (documented limit).
fn rust_imports(text: &str) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line_no = (i + 1) as u64;
        let trimmed = lines[i].trim();
        let is_use = trimmed.starts_with("use ") || trimmed.starts_with("pub use ");
        if !is_use {
            i += 1;
            continue;
        }
        let mut stmt = trimmed.to_owned();
        while !stmt.contains(';') && i + 1 < lines.len() {
            i += 1;
            stmt.push(' ');
            stmt.push_str(lines[i].trim());
        }
        i += 1;
        let Some(body) = stmt
            .split_once("use ")
            .map(|(_, rest)| rest.split(';').next().unwrap_or(rest).trim())
        else {
            continue;
        };
        let Some(path) = body.strip_prefix("crate::") else {
            continue;
        };
        if let Some((prefix, group)) = path.split_once('{') {
            let prefix = prefix.trim_end_matches("::");
            for item in group.trim_end_matches('}').split(',') {
                let item = clean_segment(item);
                if item.is_empty() || item.contains('{') {
                    continue; // nested groups: not expanded in v1
                }
                let target = if item == "self" {
                    module_path(prefix)
                } else {
                    module_path(&format!("{prefix}::{item}"))
                };
                out.push((line_no, target));
            }
        } else {
            out.push((line_no, module_path(clean_segment(path))));
        }
    }
    out
}

/// `a::b::C as D` → the `src/`-rooted path of `a::b::C`.
fn module_path(rust_path: &str) -> String {
    let segments: Vec<&str> = clean_segment(rust_path)
        .split("::")
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "*")
        .collect();
    format!("src/{}", segments.join("/"))
}

/// Trim whitespace and drop an `as` rename.
fn clean_segment(item: &str) -> &str {
    let item = item.trim();
    item.split(" as ").next().unwrap_or(item).trim()
}

/// `import a.b`, `import a.b as x, c.d`, `from a.b import c, d`,
/// `from .sib import x` (relative to the importing file's directory).
fn python_imports(rel_path: &str, text: &str) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line_no = (i + 1) as u64;
        let trimmed = raw.trim();
        if let Some(rest) = trimmed.strip_prefix("import ") {
            for item in rest.split(',') {
                let module = clean_py(item);
                if !module.is_empty() {
                    out.push((line_no, module.replace('.', "/")));
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("from ") {
            let Some((module, names)) = rest.split_once(" import ") else {
                continue;
            };
            let module = module.trim();
            let base = if let Some(after_dots) = module.strip_prefix('.') {
                let dots = 1 + after_dots.chars().take_while(|c| *c == '.').count();
                let tail = &module[dots..];
                let Some(dir) = ancestor_dir(rel_path, dots) else {
                    continue; // relative import escaping the stack root
                };
                join_module(&dir, tail)
            } else {
                module.replace('.', "/")
            };
            for name in names.split(',') {
                let name = clean_py(name);
                if name.is_empty() || name == "*" {
                    out.push((line_no, base.clone()));
                } else {
                    out.push((line_no, format!("{base}/{name}")));
                }
            }
        }
    }
    out
}

fn clean_py(item: &str) -> String {
    let item = item.trim().trim_end_matches('\\').trim();
    item.split(" as ")
        .next()
        .unwrap_or(item)
        .trim()
        .trim_matches(|c| c == '(' || c == ')')
        .to_owned()
}

/// The importing file's directory raised `levels - 1` times (`from .` =
/// the file's own package directory).
fn ancestor_dir(rel_path: &str, levels: usize) -> Option<String> {
    let mut dir: Vec<&str> = rel_path.split('/').collect();
    dir.pop(); // the file itself
    for _ in 1..levels {
        dir.pop()?;
    }
    Some(dir.join("/"))
}

fn join_module(dir: &str, tail: &str) -> String {
    let tail = tail.trim().replace('.', "/");
    match (dir.is_empty(), tail.is_empty()) {
        (_, true) => dir.to_owned(),
        (true, false) => tail,
        (false, false) => format!("{dir}/{tail}"),
    }
}

/// Relative `import`/`export … from '…'` and `require('…')` specifiers,
/// resolved against the importing file's directory.
fn ts_imports(rel_path: &str, text: &str) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line_no = (i + 1) as u64;
        let trimmed = raw.trim();
        let mut specs: Vec<&str> = Vec::new();
        if trimmed.starts_with("import ") || trimmed.starts_with("export ") {
            if let Some(spec) = quoted_after(trimmed, "from") {
                specs.push(spec);
            } else if let Some(spec) = trimmed
                .strip_prefix("import ")
                .and_then(|r| r.trim().strip_prefix(['"', '\'']))
            {
                // Side-effect import: `import './setup';`
                specs.push(spec.trim_end_matches([';', '"', '\'']));
            }
        }
        for token in ["require(", "import("] {
            if let Some(pos) = trimmed.find(token) {
                let rest = trimmed[pos + token.len()..].trim_start();
                if let Some(spec) = rest.strip_prefix(['"', '\''])
                    && let Some(end) = spec.find(['"', '\''])
                {
                    specs.push(&spec[..end]);
                }
            }
        }
        for spec in specs {
            if !spec.starts_with("./") && !spec.starts_with("../") {
                continue; // package import — external to the stack
            }
            if let Some(resolved) = resolve_relative(rel_path, spec) {
                out.push((line_no, resolved));
            }
        }
    }
    out
}

/// The quoted string following the word `key` on the line, if any.
fn quoted_after<'t>(line: &'t str, key: &str) -> Option<&'t str> {
    let pos = line.find(&format!("{key} "))?;
    let rest = line[pos + key.len()..].trim_start();
    let spec = rest.strip_prefix(['"', '\''])?;
    let end = spec.find(['"', '\''])?;
    Some(&spec[..end])
}

/// Resolve `spec` (starting with `./` or `../`) against the directory of
/// `rel_path`; `None` when it escapes the stack root.
fn resolve_relative(rel_path: &str, spec: &str) -> Option<String> {
    let mut parts: Vec<&str> = rel_path.split('/').collect();
    parts.pop(); // the file itself
    for seg in spec.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

/// `import Module` lines mapped through the `Package.swift` target table.
fn swift_imports(text: &str, targets: &BTreeMap<String, String>) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.trim();
        let Some(module) = trimmed.strip_prefix("import ") else {
            continue;
        };
        // `import struct Foo.Bar` → the module is the first path segment
        // of the last token.
        let token = module.split_whitespace().last().unwrap_or(module);
        let module = token.split('.').next().unwrap_or(token);
        if let Some(path) = targets.get(module) {
            out.push(((i + 1) as u64, path.clone()));
        }
    }
    out
}

/// Target name → source directory from `Package.swift` (textual: `name:`
/// and optional `path:` inside each `.target(`/`.executableTarget(`/
/// `.testTarget(` clause).
fn swift_target_map(root: &Path, cwd: Option<&str>) -> BTreeMap<String, String> {
    let dir = cwd.map_or_else(|| root.to_path_buf(), |c| root.join(c));
    let Ok(text) = std::fs::read_to_string(dir.join("Package.swift")) else {
        return BTreeMap::new();
    };
    parse_swift_targets(&text)
}

fn parse_swift_targets(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (token, default_dir) in [
        (".target(", "Sources"),
        (".executableTarget(", "Sources"),
        (".testTarget(", "Tests"),
    ] {
        let mut from = 0;
        while let Some(rel) = text[from..].find(token) {
            let start = from + rel + token.len();
            // The clause ends at the next target token or end of text —
            // good enough to scope `name:`/`path:` lookups.
            let end = text[start..]
                .find(".target(")
                .or_else(|| text[start..].find(".executableTarget("))
                .or_else(|| text[start..].find(".testTarget("))
                .map_or(text.len(), |e| start + e);
            let clause = &text[start..end];
            if let Some(name) = quoted_value(clause, "name:") {
                let path = quoted_value(clause, "path:")
                    .map_or_else(|| format!("{default_dir}/{name}"), str::to_owned);
                map.insert(name.to_owned(), path);
            }
            from = end;
        }
    }
    map
}

/// The first `key "value"` string after `key` in `clause`.
fn quoted_value<'t>(clause: &'t str, key: &str) -> Option<&'t str> {
    let pos = clause.find(key)?;
    let rest = clause[pos + key.len()..].trim_start();
    let spec = rest.strip_prefix('"')?;
    let end = spec.find('"')?;
    Some(&spec[..end])
}

/// `source path` / `. path` lines, resolved against the sourcing file's
/// directory. `$`-expansions are skipped (not resolvable textually).
fn bash_imports(rel_path: &str, text: &str) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.trim();
        let target = trimmed
            .strip_prefix("source ")
            .or_else(|| trimmed.strip_prefix(". "))
            .map(|rest| rest.split_whitespace().next().unwrap_or(""))
            .map(|t| t.trim_matches(['"', '\'']));
        let Some(target) = target else { continue };
        if target.is_empty() || target.contains('$') || target.starts_with('/') {
            continue;
        }
        let spec = if target.starts_with("./") || target.starts_with("../") {
            target.to_owned()
        } else {
            format!("./{target}")
        };
        if let Some(resolved) = resolve_relative(rel_path, &spec) {
            out.push(((i + 1) as u64, resolved));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_parse_and_refuse_bad_shapes() {
        let rules = parse_rules(&["src/verify -> src/gates".to_owned()]).expect("parses");
        assert_eq!(rules[0].from, "src/verify");
        assert_eq!(rules[0].to, "src/gates");
        assert!(matches!(
            parse_rules(&[]),
            Err(GateError::NotConfigured { gate: "arch", .. })
        ));
        assert!(matches!(
            parse_rules(&["src/a".to_owned()]),
            Err(GateError::BadArchRule { .. })
        ));
        assert!(matches!(
            parse_rules(&["-> src/b".to_owned()]),
            Err(GateError::BadArchRule { .. })
        ));
    }

    #[test]
    fn prefix_matching_is_extension_blind() {
        assert!(under("src/verify/mod.rs", "src/verify"));
        assert!(under("src/verify.rs", "src/verify"));
        assert!(under("src/gates", "src/gates"));
        assert!(under("src/gates/health/dup.rs", "src/gates"));
        assert!(!under("src/verifyx/mod.rs", "src/verify"));
        assert!(!under("src/gate.rs", "src/gates"));
    }

    #[test]
    fn rust_use_statements_resolve_to_src_paths() {
        let src = "use crate::gates::GateError;\npub use crate::verify::normalize::Status;\nuse crate::config::{Config, GateMode as GM};\nuse std::fmt;\nuse super::adapter;\nuse crate::plan::{\n    self,\n    PlanError,\n};\n";
        let imports = rust_imports(src);
        let targets: Vec<&str> = imports.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(
            targets,
            vec![
                "src/gates/GateError",
                "src/verify/normalize/Status",
                "src/config/Config",
                "src/config/GateMode",
                "src/plan",
                "src/plan/PlanError",
            ],
            "std/super imports are ignored; groups and multi-line joins expand"
        );
        assert_eq!(imports[0].0, 1);
        assert_eq!(imports[4].0, 6, "multi-line use keeps its first line");
    }

    #[test]
    fn python_imports_resolve_absolute_and_relative() {
        let src = "import app.db\nfrom app.models import user, order\nfrom . import sibling\nfrom ..core import engine\nimport os\n";
        let imports = python_imports("app/api/routes.py", src);
        let targets: Vec<&str> = imports.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(
            targets,
            vec![
                "app/db",
                "app/models/user",
                "app/models/order",
                "app/api/sibling",
                "app/core/engine",
                "os",
            ]
        );
    }

    #[test]
    fn ts_imports_resolve_relative_specs_only() {
        let src = "import { a } from './util';\nimport x from '../db/client';\nimport 'react';\nimport './side-effect';\nconst d = require('../lib/thing');\n";
        let imports = ts_imports("web/src/pages/home.ts", src);
        let targets: Vec<&str> = imports.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(
            targets,
            vec![
                "web/src/pages/util",
                "web/src/db/client",
                "web/src/pages/side-effect",
                "web/src/lib/thing",
            ]
        );
    }

    #[test]
    fn swift_imports_map_through_package_targets() {
        let pkg = r#"
            let package = Package(
                targets: [
                    .target(name: "Domain"),
                    .target(name: "Infra", path: "Custom/Infra"),
                    .testTarget(name: "DomainTests"),
                ]
            )
        "#;
        let targets = parse_swift_targets(pkg);
        assert_eq!(targets["Domain"], "Sources/Domain");
        assert_eq!(targets["Infra"], "Custom/Infra");
        assert_eq!(targets["DomainTests"], "Tests/DomainTests");

        let imports = swift_imports(
            "import Foundation\nimport Infra\nimport struct Domain.User\n",
            &targets,
        );
        let got: Vec<&str> = imports.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(got, vec!["Custom/Infra", "Sources/Domain"]);
    }

    #[test]
    fn bash_source_lines_resolve_against_the_file_dir() {
        let src =
            "source ./lib/common.sh\n. helpers.sh\nsource \"$HOME/x.sh\"\nsource /etc/profile\n";
        let imports = bash_imports("scripts/run.sh", src);
        let targets: Vec<&str> = imports.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(targets, vec!["scripts/lib/common.sh", "scripts/helpers.sh"]);
    }

    #[test]
    fn run_flags_a_denied_dependency_in_a_fixture_repo() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::create_dir_all(root.join("src/a")).expect("mkdirs");
        std::fs::create_dir_all(root.join("src/b")).expect("mkdirs");
        std::fs::write(
            root.join("craftsman.toml"),
            "[project]\nname = \"x\"\nstacks = [\"rust\"]\n[arch]\ndeny = [\"src/a -> src/b\"]\n",
        )
        .expect("write");
        std::fs::write(root.join("src/a/mod.rs"), "use crate::b::helper;\n").expect("write");
        std::fs::write(root.join("src/b/mod.rs"), "pub fn helper() {}\n").expect("write");
        for args in [&["init", "--quiet"][..], &["add", "-A"][..]] {
            let ok = std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                .status()
                .expect("git")
                .success();
            assert!(ok, "git {args:?}");
        }
        let config = crate::config::Config::load(root).expect("config").config;
        let outcome = run(root, &config, None, GateMode::Strict).expect("run");
        assert!(!outcome.passed());
        assert_eq!(outcome.blocking.len(), 1);
        assert_eq!(outcome.blocking[0].file, "src/a/mod.rs");
        assert!(outcome.blocking[0].message.contains("src/a -> src/b"));
    }
}

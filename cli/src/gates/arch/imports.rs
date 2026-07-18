//! Per-language textual import extraction for the arch gate: rust `use
//! crate::`, python import/from, ts relative imports (static + dynamic),
//! swift modules via Package.swift targets, bash `source`.

use std::collections::BTreeMap;
use std::path::Path;

use super::swift_targets::swift_imports;

/// Languages the extractors understand, keyed by stack name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StackLang {
    Rust,
    Python,
    Ts,
    Swift,
    Bash,
}

impl StackLang {
    pub(super) fn for_stack(stack: &str) -> Option<Self> {
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
    pub(super) fn owns(self, path: &str) -> bool {
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
pub(super) fn extract_imports(
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
        for spec in ts_line_specs(raw.trim()) {
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

/// Every module specifier on one line: static `import`/`export … from`,
/// side-effect `import './x'`, and dynamic `require(...)`/`import(...)`.
fn ts_line_specs(trimmed: &str) -> Vec<&str> {
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
    specs
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
    fn bash_source_lines_resolve_against_the_file_dir() {
        let src =
            "source ./lib/common.sh\n. helpers.sh\nsource \"$HOME/x.sh\"\nsource /etc/profile\n";
        let imports = bash_imports("scripts/run.sh", src);
        let targets: Vec<&str> = imports.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(targets, vec!["scripts/lib/common.sh", "scripts/helpers.sh"]);
    }
}

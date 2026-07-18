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

mod imports;
mod swift_targets;

use std::collections::BTreeMap;
use std::path::Path;

use imports::{StackLang, extract_imports};
use swift_targets::swift_target_map;

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

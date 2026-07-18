//! `craftsman health` — code-health metrics, own implementation.
//!
//! No external service. The evidence base (production-grade research
//! doc): function length, file length, complexity, and duplication are
//! the entropy metrics that measurably track agentic erosion;
//! CodeScene-style health scores correlate with defect density. v1 is
//! deliberately transparent and deterministic over clever.
//!
//! Metrics per tracked source file (all stacks; `git ls-files` is the file
//! census):
//!
//! - **file length**: raw line count vs `[health] max-file-lines` (400).
//!   This is health's metric — the design-doc sketch placed
//!   `max-file-lines` under `[gates.arch.rules]`; ADR-004 corrects that
//!   (arch = dependency direction only).
//! - **function length** vs `max-function-lines` (60): function headers
//!   are found textually per language (`fn`/`def`/`func`/`function`/
//!   `name() {`), bodies by brace counting (indentation for Python).
//! - **complexity** vs `max-complexity` (12): a cyclomatic approximation —
//!   1 + count of branch keywords (`if`/`for`/`while`/`match`…, `&&`,
//!   `||`) inside the function body.
//! - **duplication**: shingling over normalized lines (trimmed, blanks and
//!   comment-only and punctuation-only lines dropped); any window of
//!   `dup-window` (12) consecutive normalized lines appearing at a second
//!   location (cross-file, or ≥ window lines apart in the same file) is a
//!   duplicate block; overlapping windows merge into one finding.
//!
//! Documented accuracy limits (v1, textual — no parsers): braces inside
//! string literals or block comments miscount function extents; TS/JS
//! class methods without the `function` keyword or an arrow assignment are
//! not seen; nested named functions are measured inside their parent's
//! span too; branch keywords inside strings count toward complexity. These
//! trade exactness for zero dependencies and full determinism.
//!
//! Finding messages deliberately exclude the measured value (only the
//! threshold): baseline fingerprints hash the message, so a stable message
//! keeps the ratchet rewarding improvement (a 80→70-line function must not
//! resurface as a "new" finding) while any threshold change re-fingerprints
//! honestly.
//!
//! `--changed` narrows the *reported* findings to changed files; the scan
//! itself always covers the repo (duplication is a cross-file property).
//!
//! Inline suppression (Batch 9c): a comment line
//! `// craftsman-health: allow <rule> — <reason>` (`#` comments in
//! python/bash) suppresses one finding of `<rule>`. For the line-scoped
//! rules the directive covers the next code line below it (blank lines,
//! comments, and `#[`/`@` annotations may sit between — doc comments and
//! attributes do not break the link); `max-file-lines` is file-scoped, the
//! directive may sit anywhere (conventionally the top). The reason is
//! mandatory: a directive without one, or naming an unknown rule, is
//! itself a finding (`allow-directive`) — no naked suppressions.

use std::path::Path;

use super::{Finding, GateError, GateOutcome, Severity, epilogue};

mod allows;
mod duplication;
mod metrics;

use crate::config::{Config, GateMode};
use allows::{AllowDirective, allow_directives, apply_allows};
use duplication::{duplication_findings, normalize};
use metrics::{FunctionSpan, functions};

/// The gate/tool name for findings and baselines.
const TOOL: &str = "health";

/// Run the health gate.
///
/// # Errors
/// [`GateError`] when the file census (git) or a file read fails — a
/// broken scan is never a green gate.
pub fn run(
    root: &Path,
    config: &Config,
    changed: Option<&[String]>,
    mode: GateMode,
) -> Result<GateOutcome, GateError> {
    let mut notes: Vec<String> = Vec::new();
    let mut files = source_files(root)?;
    // Central scope exclusion applied to the census itself: excluded trees
    // must not even feed the cross-file duplication index.
    files.retain(|(path, _)| !super::scope::is_excluded(&config.gates.exclude, path));
    eprintln!(
        "gate health: scanning {} tracked source file(s) …",
        files.len()
    );

    let mut parsed: Vec<SourceFile> = Vec::new();
    for (path, lang) in files {
        let text = std::fs::read_to_string(root.join(&path)).map_err(|source| GateError::Io {
            path: root.join(&path),
            source,
        })?;
        parsed.push(SourceFile::analyze(path, lang, &text));
    }

    let mut findings = metric_findings(&parsed, config);
    findings.extend(duplication_findings(&parsed, config.health.dup_window()));
    let suppressed = apply_allows(&parsed, &mut findings);
    if suppressed > 0 {
        notes.push(format!(
            "health: {suppressed} finding(s) suppressed by inline allow \
             directives (each carries a reason)"
        ));
    }

    if let Some(changed_set) = changed {
        let before = findings.len();
        findings.retain(|f| changed_set.iter().any(|c| c == &f.file));
        notes.push(format!(
            "health: full-repo scan, findings narrowed to changed files \
             ({before} → {})",
            findings.len()
        ));
    }
    findings.sort_by(|a, b| (&a.file, a.line, &a.rule).cmp(&(&b.file, b.line, &b.rule)));

    epilogue::finish(
        &epilogue::Epilogue {
            root,
            config,
            gate: "health",
            changed,
            mode,
        },
        findings,
        notes,
        vec![TOOL],
    )
}

/// Languages the heuristics understand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Lang {
    Rust,
    Python,
    Ts,
    Swift,
    Bash,
}

impl Lang {
    fn from_path(path: &str) -> Option<Self> {
        let ext = Path::new(path).extension()?.to_str()?;
        match ext {
            "rs" => Some(Self::Rust),
            "py" => Some(Self::Python),
            "ts" | "tsx" | "js" | "jsx" => Some(Self::Ts),
            "swift" => Some(Self::Swift),
            "sh" | "bash" => Some(Self::Bash),
            _ => None,
        }
    }

    /// Line-comment prefixes — used to skip comment-only lines.
    const fn comment_prefixes(self) -> &'static [&'static str] {
        match self {
            Self::Rust | Self::Swift => &["//", "/*", "*", "*/"],
            Self::Ts => &["//", "/*", "*", "*/", "///"],
            Self::Python | Self::Bash => &["#"],
        }
    }

    /// Branch keywords for the complexity approximation.
    // craftsman-health: allow max-complexity — keyword table: the words are data here, and the scanner counts its own vocabulary as branches
    const fn branch_words(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["if", "while", "for", "match"],
            Self::Python => &["if", "elif", "for", "while", "and", "or", "except", "case"],
            Self::Ts => &["if", "for", "while", "case", "catch"],
            Self::Swift => &["if", "guard", "for", "while", "case", "catch"],
            Self::Bash => &["if", "elif", "for", "while", "case"],
        }
    }

    /// Whether `&&` / `||` count as branches (Python spells them `and`/`or`).
    const fn counts_logical_ops(self) -> bool {
        !matches!(self, Self::Python)
    }
}

/// One analyzed file: metrics plus the normalized lines for shingling.
#[derive(Debug)]
struct SourceFile {
    path: String,
    line_count: usize,
    functions: Vec<FunctionSpan>,
    /// (1-based line, normalized text) — duplication input.
    normalized: Vec<(u64, String)>,
    /// Inline `craftsman-health: allow` directives.
    allows: Vec<AllowDirective>,
}

impl SourceFile {
    fn analyze(path: String, lang: Lang, text: &str) -> Self {
        let lines: Vec<&str> = text.lines().collect();
        Self {
            functions: functions(lang, &lines),
            normalized: normalize(lang, &lines),
            allows: allow_directives(lang, &lines),
            line_count: lines.len(),
            path,
        }
    }
}

/// Tracked files the heuristics understand, sorted (deterministic output).
fn source_files(root: &Path) -> Result<Vec<(String, Lang)>, GateError> {
    let mut files: Vec<(String, Lang)> = super::git(root, &["ls-files"])?
        .lines()
        .filter_map(|p| Lang::from_path(p).map(|l| (p.to_owned(), l)))
        .collect();
    files.sort();
    Ok(files)
}

/// Size and complexity findings for every analyzed file.
fn metric_findings(parsed: &[SourceFile], config: &Config) -> Vec<Finding> {
    let health = &config.health;
    let mut findings = Vec::new();
    for file in parsed {
        if file.line_count > health.max_file_lines() {
            findings.push(finding(
                "max-file-lines",
                &file.path,
                None,
                format!("file exceeds max-file-lines {}", health.max_file_lines()),
            ));
        }
        for f in &file.functions {
            if f.lines > health.max_function_lines() {
                findings.push(finding(
                    "max-function-lines",
                    &file.path,
                    Some(f.start),
                    format!(
                        "function `{}` exceeds max-function-lines {}",
                        f.name,
                        health.max_function_lines()
                    ),
                ));
            }
            if f.complexity > health.max_complexity() {
                findings.push(finding(
                    "max-complexity",
                    &file.path,
                    Some(f.start),
                    format!(
                        "function `{}` exceeds max-complexity {} \
                         (branch-keyword approximation)",
                        f.name,
                        health.max_complexity()
                    ),
                ));
            }
        }
    }
    findings
}

fn finding(rule: &str, file: &str, line: Option<u64>, message: String) -> Finding {
    Finding {
        gate: "health",
        tool: TOOL,
        rule: rule.to_owned(),
        file: file.to_owned(),
        line,
        message,
        severity: Severity::Medium,
    }
}

fn is_comment(lang: Lang, trimmed: &str) -> bool {
    lang.comment_prefixes()
        .iter()
        .any(|p| trimmed.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn thresholds_produce_findings() {
        let mut config = crate::config::Config::from_toml(
            "[project]\nname = \"x\"\nstacks = [\"rust\"]\n[health]\nmax-function-lines = 3\nmax-complexity = 2\nmax-file-lines = 5\n",
            Path::new("craftsman.toml"),
        )
        .expect("config parses");
        let src = "fn long(x: i32) -> i32 {\n    if x > 1 {\n        return 1;\n    }\n    if x > 2 {\n        return 2;\n    }\n    0\n}\n";
        let parsed = vec![SourceFile::analyze("src/a.rs".to_owned(), Lang::Rust, src)];
        let findings = metric_findings(&parsed, &config);
        let rules: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
        assert!(rules.contains(&"max-function-lines"), "{rules:?}");
        assert!(rules.contains(&"max-complexity"), "{rules:?}");
        assert!(rules.contains(&"max-file-lines"), "{rules:?}");
        assert!(
            findings.iter().all(|f| !f.message.contains('9')),
            "messages carry thresholds, never measured values: {findings:?}"
        );

        config.health.max_function_lines = Some(60);
        config.health.max_complexity = Some(12);
        config.health.max_file_lines = Some(400);
        assert!(metric_findings(&parsed, &config).is_empty());
    }
}

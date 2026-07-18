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

use std::collections::BTreeMap;
use std::path::Path;

use super::{Finding, GateError, GateOutcome, Severity, epilogue, fnv_hex};
use crate::config::{Config, GateMode};

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

/// One measured function.
#[derive(Debug)]
struct FunctionSpan {
    name: String,
    /// 1-based line of the header.
    start: u64,
    lines: usize,
    complexity: usize,
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

// ---------------------------------------------------------------- functions

/// Extract measured functions from a file.
fn functions(lang: Lang, lines: &[&str]) -> Vec<FunctionSpan> {
    let mut spans = Vec::new();
    for (i, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim_start();
        if is_comment(lang, trimmed) {
            continue;
        }
        let Some(name) = function_name(lang, trimmed) else {
            continue;
        };
        let Some(end) = body_end(lang, lines, i) else {
            continue; // declaration without a body (trait fn, protocol)
        };
        let body = &lines[i..=end];
        spans.push(FunctionSpan {
            name,
            start: (i + 1) as u64,
            lines: body.len(),
            complexity: complexity(lang, body),
        });
    }
    spans
}

/// The function name when `trimmed` looks like a function header.
fn function_name(lang: Lang, trimmed: &str) -> Option<String> {
    match lang {
        Lang::Rust => name_after_keyword(trimmed, "fn"),
        Lang::Python => {
            let rest = trimmed
                .strip_prefix("async def ")
                .or_else(|| trimmed.strip_prefix("def "))?;
            Some(ident_prefix(rest))
        }
        Lang::Swift => name_after_keyword(trimmed, "func"),
        Lang::Ts => {
            if let Some(name) = name_after_keyword(trimmed, "function") {
                return Some(name);
            }
            // `const name = (…) =>` / `let name = async (…) =>`
            let rest = ["const ", "let ", "var "]
                .iter()
                .find_map(|kw| trimmed.strip_prefix(kw))?;
            let (name, tail) = rest.split_once('=')?;
            tail.contains("=>").then(|| ident_prefix(name.trim()))
        }
        Lang::Bash => {
            if let Some(rest) = trimmed.strip_prefix("function ") {
                return Some(ident_prefix(rest));
            }
            // `name() {`
            let open = trimmed.find("()")?;
            let name = &trimmed[..open];
            (!name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && trimmed[open..].contains('{'))
            .then(|| name.to_owned())
        }
    }
}

/// The identifier following a whole-word `keyword` in `trimmed`, if any
/// (`pub const fn name`, `override func name`, `export async function name`).
fn name_after_keyword(trimmed: &str, keyword: &str) -> Option<String> {
    let pos = find_word(trimmed, keyword)?;
    let rest = trimmed[pos + keyword.len()..].trim_start();
    let name = ident_prefix(rest);
    (!name.is_empty()).then_some(name)
}

/// Leading identifier characters of `text`.
fn ident_prefix(text: &str) -> String {
    text.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Last line index of the function body starting at `header`, or `None`
/// for a body-less declaration.
fn body_end(lang: Lang, lines: &[&str], header: usize) -> Option<usize> {
    if lang == Lang::Python {
        return python_body_end(lines, header);
    }
    // Brace languages: find the opening `{` on the header or within the
    // next few lines (multi-line signatures), then balance braces. Naive
    // counting — braces in strings/comments miscount (module docs).
    let mut depth: i64 = 0;
    let mut opened = false;
    for (i, line) in lines.iter().enumerate().skip(header) {
        if !opened && i > header + 4 {
            return None; // no body in sight: a declaration
        }
        if !opened && line.contains(';') && !line.contains('{') {
            return None; // `fn x();` — trait/protocol declaration
        }
        for c in line.chars() {
            match c {
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        if opened && depth <= 0 {
            return Some(i);
        }
    }
    None
}

/// Python: the body is every following line blank or indented deeper than
/// the header; trailing blanks are not counted.
fn python_body_end(lines: &[&str], header: usize) -> Option<usize> {
    let indent = indent_width(lines[header]);
    let mut last = header;
    for (i, line) in lines.iter().enumerate().skip(header + 1) {
        if line.trim().is_empty() {
            continue;
        }
        if indent_width(line) <= indent {
            break;
        }
        last = i;
    }
    (last > header).then_some(last)
}

fn indent_width(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

// --------------------------------------------------------------- complexity

/// 1 + branch keywords in the span (comment-only lines skipped).
fn complexity(lang: Lang, body: &[&str]) -> usize {
    let mut count = 1;
    for raw in body {
        let trimmed = raw.trim_start();
        if is_comment(lang, trimmed) {
            continue;
        }
        for word in lang.branch_words() {
            count += count_word(trimmed, word);
        }
        if lang.counts_logical_ops() {
            count += trimmed.matches("&&").count() + trimmed.matches("||").count();
        }
    }
    count
}

fn is_comment(lang: Lang, trimmed: &str) -> bool {
    lang.comment_prefixes()
        .iter()
        .any(|p| trimmed.starts_with(p))
}

/// Whole-word occurrences of `word` in `text`.
fn count_word(text: &str, word: &str) -> usize {
    find_word_all(text, word).len()
}

/// Byte offset of the first whole-word occurrence.
fn find_word(text: &str, word: &str) -> Option<usize> {
    find_word_all(text, word).first().copied()
}

fn find_word_all(text: &str, word: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut hits = Vec::new();
    let mut from = 0;
    while let Some(rel) = text[from..].find(word) {
        let start = from + rel;
        let end = start + word.len();
        let left_ok = start == 0 || !is_ident(bytes[start - 1]);
        let right_ok = end == bytes.len() || !is_ident(bytes[end]);
        if left_ok && right_ok {
            hits.push(start);
        }
        from = end;
    }
    hits
}

// ------------------------------------------------------------------- allows

/// The inline-suppression marker, always inside a comment line.
const ALLOW_MARKER: &str = "craftsman-health: allow";

/// Rules a directive may suppress.
const ALLOWABLE_RULES: &[&str] = &[
    "max-function-lines",
    "max-file-lines",
    "max-complexity",
    "duplication",
];

/// One parsed `craftsman-health: allow <rule> — <reason>` comment.
#[derive(Debug)]
struct AllowDirective {
    /// 1-based line the comment sits on.
    line: u64,
    rule: String,
    /// Empty = invalid (surfaced as an `allow-directive` finding).
    reason: String,
    /// 1-based first code line below the comment — what a line-scoped
    /// directive covers. Blanks, comments, and `#[`/`@` annotations in
    /// between do not break the link.
    target: Option<u64>,
}

impl AllowDirective {
    fn is_valid(&self) -> bool {
        !self.reason.is_empty() && ALLOWABLE_RULES.contains(&self.rule.as_str())
    }

    /// Whether this directive suppresses a finding of `rule` at `line`.
    fn covers(&self, rule: &str, line: Option<u64>) -> bool {
        self.is_valid()
            && self.rule == rule
            && (rule == "max-file-lines" || (line.is_some() && line == self.target))
    }
}

/// Parse every allow directive in a file.
fn allow_directives(lang: Lang, lines: &[&str]) -> Vec<AllowDirective> {
    let mut out = Vec::new();
    for (i, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim_start();
        if !is_comment(lang, trimmed) {
            continue;
        }
        // The directive must START the comment (`// craftsman-health: …`).
        // Mentions elsewhere in a comment — like this gate's own docs —
        // are prose, not suppressions.
        let after = trimmed
            .trim_start_matches(['/', '#', '*', '!'])
            .trim_start();
        let Some(rest) = after.strip_prefix(ALLOW_MARKER) else {
            continue;
        };
        let rest = rest.trim_start();
        let (rule, reason_part) = rest
            .split_once(|c: char| c.is_whitespace() || c == '—')
            .unwrap_or((rest, ""));
        let reason = reason_part
            .trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '—' | '–' | '-'))
            .trim_end();
        let target = lines
            .iter()
            .enumerate()
            .skip(i + 1)
            .find(|(_, l)| {
                let t = l.trim_start();
                !t.is_empty() && !is_comment(lang, t) && !t.starts_with("#[") && !t.starts_with('@')
            })
            .map(|(j, _)| (j + 1) as u64);
        out.push(AllowDirective {
            line: (i + 1) as u64,
            rule: rule.to_owned(),
            reason: reason.to_owned(),
            target,
        });
    }
    out
}

/// Drop findings covered by a valid directive; append an `allow-directive`
/// finding for every invalid one (missing reason, unknown rule). Returns
/// the suppressed count.
fn apply_allows(parsed: &[SourceFile], findings: &mut Vec<Finding>) -> usize {
    let before = findings.len();
    findings.retain(|f| {
        !parsed
            .iter()
            .find(|p| p.path == f.file)
            .is_some_and(|p| p.allows.iter().any(|d| d.covers(&f.rule, f.line)))
    });
    let suppressed = before - findings.len();
    for file in parsed {
        for d in file.allows.iter().filter(|d| !d.is_valid()) {
            let detail = if ALLOWABLE_RULES.contains(&d.rule.as_str()) {
                format!("allow directive for `{}` has no reason — say why", d.rule)
            } else {
                format!("allow directive names unknown rule `{}`", d.rule)
            };
            findings.push(finding("allow-directive", &file.path, Some(d.line), detail));
        }
    }
    suppressed
}

// -------------------------------------------------------------- duplication

/// Duplicate-block findings across every analyzed file.
fn duplication_findings(parsed: &[SourceFile], window: usize) -> Vec<Finding> {
    if window == 0 {
        return Vec::new();
    }
    // hash → every (file index, entry index) where the window occurs.
    let mut index: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
    for (fi, file) in parsed.iter().enumerate() {
        if file.normalized.len() < window {
            continue;
        }
        for start in 0..=(file.normalized.len() - window) {
            let key = fnv_hex(
                &file.normalized[start..start + window]
                    .iter()
                    .map(|(_, text)| text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            index.entry(key).or_default().push((fi, start));
        }
    }

    // Per file: duplicated entry indexes + a partner path per window start.
    let mut marked: BTreeMap<usize, BTreeMap<usize, usize>> = BTreeMap::new();
    for occurrences in index.values() {
        if occurrences.len() < 2 {
            continue;
        }
        for &(fi, start) in occurrences {
            // A partner is any occurrence in another file, or one at least
            // `window` entries away in the same file (self-overlap of
            // repetitive code is not duplication).
            let partner = occurrences
                .iter()
                .find(|&&(pfi, ps)| pfi != fi || ps.abs_diff(start) >= window);
            if let Some(&(pfi, _)) = partner {
                marked.entry(fi).or_default().insert(start, pfi);
            }
        }
    }

    let mut findings = Vec::new();
    for (fi, starts) in &marked {
        let file = &parsed[*fi];
        // Merge overlapping/adjacent windows into runs.
        let mut run: Option<(usize, usize, usize)> = None; // (start, end, partner)
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for (&start, &partner) in starts {
            match run {
                Some((rs, re, rp)) if start <= re + 1 => {
                    run = Some((rs, start.max(re), rp));
                    let _ = partner;
                }
                Some((rs, _, rp)) => {
                    runs.push((rs, rp));
                    run = Some((start, start, partner));
                }
                None => run = Some((start, start, partner)),
            }
        }
        if let Some((rs, _, rp)) = run {
            runs.push((rs, rp));
        }
        for (entry_start, partner_fi) in runs {
            let line = file.normalized[entry_start].0;
            let partner_path = &parsed[partner_fi].path;
            let other = if partner_path == &file.path {
                "elsewhere in this file".to_owned()
            } else {
                format!("also in {partner_path}")
            };
            findings.push(finding(
                "duplication",
                &file.path,
                Some(line),
                format!("duplicated block of {window}+ normalized lines ({other})"),
            ));
        }
    }
    findings
}

/// Normalized, shingle-worthy lines: trimmed, non-blank, not comment-only,
/// and containing at least one alphanumeric character (pure punctuation
/// like `}` or `});` carries no duplication signal).
fn normalize(lang: Lang, lines: &[&str]) -> Vec<(u64, String)> {
    lines
        .iter()
        .enumerate()
        .filter_map(|(i, raw)| {
            let t = raw.trim();
            (!t.is_empty() && !is_comment(lang, t) && t.chars().any(char::is_alphanumeric))
                .then(|| ((i + 1) as u64, t.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyze(lang: Lang, text: &str) -> SourceFile {
        SourceFile::analyze("f".to_owned(), lang, text)
    }

    fn numbered_lines(n: usize) -> String {
        use std::fmt::Write as _;
        let mut block = String::new();
        for i in 0..n {
            writeln!(block, "    let v{i} = compute({i});").expect("write to string");
        }
        block
    }

    #[test]
    fn rust_functions_are_measured_with_braces() {
        let src = "pub fn short(x: i32) -> i32 {\n    if x > 0 && x < 9 {\n        x\n    } else {\n        0\n    }\n}\n\nfn other() {}\n";
        let file = analyze(Lang::Rust, src);
        assert_eq!(file.functions.len(), 2);
        assert_eq!(file.functions[0].name, "short");
        assert_eq!(file.functions[0].start, 1);
        assert_eq!(file.functions[0].lines, 7);
        // 1 + if + && = 3
        assert_eq!(file.functions[0].complexity, 3);
        assert_eq!(file.functions[1].lines, 1);
    }

    #[test]
    fn rust_trait_declarations_are_not_functions() {
        let file = analyze(Lang::Rust, "trait T {\n    fn declared(&self);\n}\n");
        assert!(file.functions.is_empty(), "{:?}", file.functions);
    }

    #[test]
    fn python_functions_are_measured_by_indentation() {
        let src = "def outer(x):\n    if x and x > 1:\n        return x\n    return 0\n\n\nclass C:\n    def method(self):\n        for i in range(3):\n            pass\n";
        let file = analyze(Lang::Python, src);
        assert_eq!(file.functions.len(), 2);
        assert_eq!(file.functions[0].name, "outer");
        assert_eq!(file.functions[0].lines, 4);
        // 1 + if + and = 3
        assert_eq!(file.functions[0].complexity, 3);
        assert_eq!(file.functions[1].name, "method");
        assert_eq!(file.functions[1].start, 8);
        assert_eq!(file.functions[1].lines, 3);
    }

    #[test]
    fn ts_swift_bash_headers_are_recognized() {
        let ts = analyze(
            Lang::Ts,
            "export function fx(a) {\n  return a;\n}\nconst arrow = (b) => {\n  return b;\n};\n",
        );
        assert_eq!(
            ts.functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            vec!["fx", "arrow"]
        );

        let swift = analyze(
            Lang::Swift,
            "override func body() -> Int {\n    return 1\n}\n",
        );
        assert_eq!(swift.functions[0].name, "body");

        let bash = analyze(
            Lang::Bash,
            "greet() {\n  echo hi\n}\nfunction bye {\n  echo bye\n}\n",
        );
        assert_eq!(
            bash.functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            vec!["greet", "bye"]
        );
    }

    #[test]
    fn branch_words_match_whole_words_only() {
        assert_eq!(count_word("before iffy modifier", "if"), 0);
        assert_eq!(count_word("if x { } else if y { }", "if"), 2);
        assert_eq!(find_word("pub fn f()", "fn"), Some(4));
        assert_eq!(find_word("pubfn f()", "fn"), None);
    }

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

    #[test]
    fn allow_directive_with_reason_suppresses_the_next_function() {
        let long_body = numbered_lines(70);
        let src = format!(
            "// craftsman-health: allow max-function-lines — generated glue, one row per fixture\n\
             /// Doc comment between directive and header stays linked.\n\
             #[must_use]\n\
             fn long() {{\n{long_body}}}\n"
        );
        let parsed = vec![SourceFile::analyze("src/a.rs".to_owned(), Lang::Rust, &src)];
        assert_eq!(parsed[0].allows.len(), 1);
        assert_eq!(
            parsed[0].allows[0].target,
            Some(4),
            "{:?}",
            parsed[0].allows
        );

        let config = default_config();
        let mut findings = metric_findings(&parsed, &config);
        assert_eq!(findings.len(), 1, "{findings:?}");
        let suppressed = apply_allows(&parsed, &mut findings);
        assert_eq!(suppressed, 1);
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn allow_without_a_reason_is_its_own_finding_and_suppresses_nothing() {
        let long_body = numbered_lines(70);
        let src =
            format!("// craftsman-health: allow max-function-lines\nfn long() {{\n{long_body}}}\n");
        let parsed = vec![SourceFile::analyze("src/a.rs".to_owned(), Lang::Rust, &src)];
        let config = default_config();
        let mut findings = metric_findings(&parsed, &config);
        let suppressed = apply_allows(&parsed, &mut findings);
        assert_eq!(suppressed, 0);
        let rules: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
        assert!(rules.contains(&"max-function-lines"), "{rules:?}");
        assert!(rules.contains(&"allow-directive"), "{rules:?}");
    }

    #[test]
    fn allow_of_an_unknown_rule_is_rejected() {
        let src = "// craftsman-health: allow max-vibes — because\nfn ok() {}\n";
        let parsed = vec![SourceFile::analyze("src/a.rs".to_owned(), Lang::Rust, src)];
        let mut findings = Vec::new();
        assert_eq!(apply_allows(&parsed, &mut findings), 0);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "allow-directive");
        assert!(findings[0].message.contains("max-vibes"), "{findings:?}");
    }

    #[test]
    fn file_scoped_allow_covers_max_file_lines_from_anywhere() {
        let body = numbered_lines(450);
        let src = format!(
            "fn a() {{\n{body}}}\n// craftsman-health: allow max-file-lines — cohesive fixture table\n"
        );
        let parsed = vec![SourceFile::analyze("src/a.rs".to_owned(), Lang::Rust, &src)];
        let config = default_config();
        let mut findings = metric_findings(&parsed, &config);
        let file_findings: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
        assert!(
            file_findings.contains(&"max-file-lines"),
            "{file_findings:?}"
        );
        let suppressed = apply_allows(&parsed, &mut findings);
        assert_eq!(suppressed, 1);
        assert!(
            findings.iter().all(|f| f.rule != "max-file-lines"),
            "{findings:?}"
        );
        // The function-length finding is untouched — allows are per-rule.
        assert!(findings.iter().any(|f| f.rule == "max-function-lines"));
    }

    fn default_config() -> crate::config::Config {
        crate::config::Config::from_toml(
            "[project]\nname = \"x\"\nstacks = [\"rust\"]\n",
            Path::new("craftsman.toml"),
        )
        .expect("config parses")
    }

    #[test]
    fn duplication_is_found_across_files_and_merged() {
        let block = numbered_lines(14);
        let a = SourceFile::analyze(
            "src/a.rs".to_owned(),
            Lang::Rust,
            &format!("fn a() {{\n{block}}}\n"),
        );
        let b = SourceFile::analyze(
            "src/b.rs".to_owned(),
            Lang::Rust,
            &format!("fn b() {{\n{block}}}\n"),
        );
        let findings = duplication_findings(&[a, b], 12);
        assert_eq!(findings.len(), 2, "{findings:?}");
        assert!(findings[0].message.contains("src/b.rs"));
        assert!(findings[1].message.contains("src/a.rs"));
        assert_eq!(findings[0].rule, "duplication");

        // Below the window: no findings.
        let short = numbered_lines(8);
        let a = SourceFile::analyze("src/a.rs".to_owned(), Lang::Rust, &short);
        let b = SourceFile::analyze("src/b.rs".to_owned(), Lang::Rust, &short);
        assert!(duplication_findings(&[a, b], 12).is_empty());
    }

    #[test]
    fn self_overlap_of_repetitive_code_is_not_duplication() {
        // 20 identical lines: windows overlap themselves at distance < 12.
        let text: String = "    step();\n".repeat(20);
        let a = SourceFile::analyze("src/a.rs".to_owned(), Lang::Rust, &text);
        assert!(duplication_findings(&[a], 12).is_empty());
    }

    #[test]
    fn normalization_drops_noise_lines() {
        let lines = ["", "  }", "// comment", "  real(code);", "});"];
        let norm = normalize(Lang::Rust, &lines);
        assert_eq!(norm, vec![(4, "real(code);".to_owned())]);
    }
}

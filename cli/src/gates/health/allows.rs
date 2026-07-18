//! Inline `craftsman-health: allow` suppression directives. Syntax and
//! scoping rules are documented on the parent module: reasons are
//! mandatory, invalid directives are findings.

use super::{Finding, Lang, SourceFile, finding, is_comment};

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
pub(super) struct AllowDirective {
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
pub(super) fn allow_directives(lang: Lang, lines: &[&str]) -> Vec<AllowDirective> {
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
pub(super) fn apply_allows(parsed: &[SourceFile], findings: &mut Vec<Finding>) -> usize {
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

#[cfg(test)]
mod tests {
    use super::super::{Lang, metric_findings};
    use super::*;

    fn numbered_lines(n: usize) -> String {
        use std::fmt::Write as _;
        let mut block = String::new();
        for i in 0..n {
            writeln!(block, "    let v{i} = compute({i});").expect("write to string");
        }
        block
    }

    fn default_config() -> crate::config::Config {
        crate::config::Config::from_toml(
            "[project]\nname = \"x\"\nstacks = [\"rust\"]\n",
            std::path::Path::new("craftsman.toml"),
        )
        .expect("config parses")
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
}

//! Cross-file duplicate-block detection: shingling over normalized lines,
//! overlapping windows merged into one finding per run. Semantics are
//! documented on the parent module.

use std::collections::BTreeMap;

use super::super::fnv_hex;
use super::{Finding, Lang, SourceFile, finding, is_comment};

/// hash → every `(file index, entry index)` where a window occurs.
type WindowIndex = BTreeMap<String, Vec<(usize, usize)>>;

/// Duplicate-block findings across every analyzed file: shingle, find
/// partnered windows, merge overlaps, report one finding per run.
pub(super) fn duplication_findings(parsed: &[SourceFile], window: usize) -> Vec<Finding> {
    if window == 0 {
        return Vec::new();
    }
    let marked = mark_partners(&window_index(parsed, window), window);
    let mut findings = Vec::new();
    for (fi, starts) in &marked {
        let file = &parsed[*fi];
        for (entry_start, partner_fi) in merge_runs(starts) {
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

/// Shingle every file's normalized lines into hashed windows.
fn window_index(parsed: &[SourceFile], window: usize) -> WindowIndex {
    let mut index = WindowIndex::new();
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
    index
}

/// Per file: duplicated entry indexes + a partner file per window start.
///
/// A partner is any occurrence in another file, or one at least `window`
/// entries away in the same file (self-overlap of repetitive code is not
/// duplication).
fn mark_partners(index: &WindowIndex, window: usize) -> BTreeMap<usize, BTreeMap<usize, usize>> {
    let mut marked: BTreeMap<usize, BTreeMap<usize, usize>> = BTreeMap::new();
    for occurrences in index.values() {
        if occurrences.len() < 2 {
            continue;
        }
        for &(fi, start) in occurrences {
            let partner = occurrences
                .iter()
                .find(|&&(pfi, ps)| pfi != fi || ps.abs_diff(start) >= window);
            if let Some(&(pfi, _)) = partner {
                marked.entry(fi).or_default().insert(start, pfi);
            }
        }
    }
    marked
}

/// Merge overlapping/adjacent window starts into `(run start, partner)`
/// pairs — one finding per contiguous duplicated block.
fn merge_runs(starts: &BTreeMap<usize, usize>) -> Vec<(usize, usize)> {
    let mut run: Option<(usize, usize, usize)> = None; // (start, end, partner)
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (&start, &partner) in starts {
        match run {
            Some((rs, re, rp)) if start <= re + 1 => {
                run = Some((rs, start.max(re), rp));
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
    runs
}

/// Normalized, shingle-worthy lines: trimmed, non-blank, not comment-only,
/// and containing at least one alphanumeric character (pure punctuation
/// like `}` or `});` carries no duplication signal).
pub(super) fn normalize(lang: Lang, lines: &[&str]) -> Vec<(u64, String)> {
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
    use super::super::SourceFile;
    use super::*;

    fn numbered_lines(n: usize) -> String {
        use std::fmt::Write as _;
        let mut block = String::new();
        for i in 0..n {
            writeln!(block, "    let v{i} = compute({i});").expect("write to string");
        }
        block
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

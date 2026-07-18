//! Gate scope exclusion — `[gates] exclude` glob patterns (Batch 9c).
//!
//! One central implementation applied to every gate: the shared epilogue
//! (`lint::finish`) drops findings in excluded paths, and gates that build
//! an explicit file census (health, security lockfiles, shellcheck) filter
//! it with the same predicate so excluded trees are never even scanned.
//!
//! Pattern language (deliberately tiny, no dependency): `/`-separated
//! segments; `*` matches within one segment; a `**` segment matches any
//! number of segments, including none. `spikes/**` therefore covers
//! `spikes/a.rs` and `spikes/a/b/c.rs` but never `spikes` itself becoming
//! a match for `spikes-other/x`.

/// Whether `path` (root-relative, `/`-separated) falls under any exclude
/// pattern.
#[must_use]
pub fn is_excluded(patterns: &[String], path: &str) -> bool {
    patterns.iter().any(|p| glob_match(p, path))
}

/// Drop excluded entries from a root-relative file list, returning how many
/// were removed (for the honest note).
pub fn filter_files(patterns: &[String], files: &mut Vec<String>) -> usize {
    let before = files.len();
    files.retain(|f| !is_excluded(patterns, f));
    before - files.len()
}

/// Segment-wise glob match: `**` spans segments, `*` wildcards within one.
fn glob_match(pattern: &str, path: &str) -> bool {
    let p: Vec<&str> = pattern.split('/').collect();
    let t: Vec<&str> = path.split('/').collect();
    match_segments(&p, &t)
}

fn match_segments(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => {
            // Trailing `**` means "everything inside" (gitignore semantics):
            // at least one segment. A middle `**` spans zero or more.
            if rest.is_empty() {
                return !path.is_empty();
            }
            match_segments(rest, path) || (!path.is_empty() && match_segments(pattern, &path[1..]))
        }
        Some((seg, rest)) => path
            .split_first()
            .is_some_and(|(head, tail)| match_one(seg, head) && match_segments(rest, tail)),
    }
}

/// `*`-wildcard match within a single segment.
fn match_one(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((prefix, rest)) => text.strip_prefix(prefix).is_some_and(|after| {
            if rest.is_empty() {
                return true;
            }
            // Try every suffix position for the remainder after `*`.
            (0..=after.len())
                .filter(|i| after.is_char_boundary(*i))
                .any(|i| match_one(rest, &after[i..]))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn excl(patterns: &[&str]) -> Vec<String> {
        patterns.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn double_star_spans_segments() {
        let p = excl(&["spikes/**"]);
        assert!(is_excluded(&p, "spikes/a.rs"));
        assert!(is_excluded(
            &p,
            "spikes/s2-normalizer/samples/rust-todo/Cargo.lock"
        ));
        assert!(!is_excluded(&p, "spikes"), "the bare directory itself");
        assert!(!is_excluded(&p, "spikes-other/a.rs"));
        assert!(!is_excluded(&p, "cli/src/spikes.rs"));
    }

    #[test]
    fn single_star_stays_within_a_segment() {
        let p = excl(&["cli/*.rs"]);
        assert!(is_excluded(&p, "cli/build.rs"));
        assert!(!is_excluded(&p, "cli/src/main.rs"));
        let p = excl(&["**/*.lock"]);
        assert!(is_excluded(&p, "Cargo.lock"));
        assert!(is_excluded(&p, "a/b/bun.lock"));
        assert!(!is_excluded(&p, "a/b/lockfile"));
    }

    #[test]
    fn empty_patterns_exclude_nothing() {
        assert!(!is_excluded(&[], "spikes/a.rs"));
    }

    #[test]
    fn filter_files_reports_the_removed_count() {
        let p = excl(&["spikes/**"]);
        let mut files = vec![
            "cli/src/main.rs".to_owned(),
            "spikes/s1/x.swift".to_owned(),
            "spikes/s2/y.rs".to_owned(),
        ];
        assert_eq!(filter_files(&p, &mut files), 2);
        assert_eq!(files, vec!["cli/src/main.rs".to_owned()]);
    }
}

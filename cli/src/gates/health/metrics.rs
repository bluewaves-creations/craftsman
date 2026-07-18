//! Function-level metrics: extraction of measured spans (textual, per
//! language) and the cyclomatic-complexity approximation. Accuracy limits
//! are documented on the parent module.

use super::{Lang, is_comment};

/// One measured function.
#[derive(Debug)]
pub(super) struct FunctionSpan {
    pub(super) name: String,
    /// 1-based line of the header.
    pub(super) start: u64,
    pub(super) lines: usize,
    pub(super) complexity: usize,
}

// ----- functions

/// Extract measured functions from a file.
pub(super) fn functions(lang: Lang, lines: &[&str]) -> Vec<FunctionSpan> {
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

// ----- complexity

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

#[cfg(test)]
mod tests {
    use super::super::SourceFile;
    use super::*;

    fn analyze(lang: Lang, text: &str) -> SourceFile {
        SourceFile::analyze("f".to_owned(), lang, text)
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
}

//! Swift import mapping for the arch gate: `import <Module>` lines
//! resolved through the Package.swift target table (module name → target
//! source directory).

use std::collections::BTreeMap;
use std::path::Path;

/// `import Module` lines mapped through the `Package.swift` target table.
pub(super) fn swift_imports(text: &str, targets: &BTreeMap<String, String>) -> Vec<(u64, String)> {
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
pub(super) fn swift_target_map(root: &Path, cwd: Option<&str>) -> BTreeMap<String, String> {
    let dir = cwd.map_or_else(|| root.to_path_buf(), |c| root.join(c));
    let Ok(text) = std::fs::read_to_string(dir.join("Package.swift")) else {
        return BTreeMap::new();
    };
    parse_swift_targets(&text)
}

pub(super) fn parse_swift_targets(text: &str) -> BTreeMap<String, String> {
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
pub(super) fn quoted_value<'t>(clause: &'t str, key: &str) -> Option<&'t str> {
    let pos = clause.find(key)?;
    let rest = clause[pos + key.len()..].trim_start();
    let spec = rest.strip_prefix('"')?;
    let end = spec.find('"')?;
    Some(&spec[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

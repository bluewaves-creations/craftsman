//! Gherkin → Swift Testing generator, implementing ADR-001's proven target
//! shape (`spikes/s1-swift-codegen/Tests/SpecSpikeTests/TodoFeature.swift`).
//!
//! `@Suite("Feature: <name>")` struct per feature, one raw-identifier
//! (SE-0451) `@Test` function per scenario — the scenario name verbatim in
//! backticks, no other mangling (ADR-001 rule 3; lint already rejected the
//! compile-breaking characters) — `.tags(...)` traits with a generated
//! `Tag` extension, and Scenario Outlines as `@Test(arguments:)` over the
//! Examples rows.
//!
//! Steps: the generated test calls `steps.step_<slug>(…)` methods on a
//! `SpecSteps` value. The stub template declares every unique step as a
//! `mutating func … throws` whose body is
//! `#expect(Bool(false), "step not implemented: <text>")` — that message
//! prefix is the Undefined marker the swift-testing adapter recognizes
//! (message-prefix dialect, see `normalize::parse_swift_events_jsonl`).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::{ExampleTable, GenError, StepRegistry, example_table, slug, typed_params};
use crate::config::VerifyStack;

/// The marker prefix generated stubs put in their `#expect` comment; the
/// adapter maps a failure carrying it to `Undefined`, not `Failed`
/// (message-prefix dialect, defined once in the normalizer).
pub use crate::verify::normalize::NOT_IMPLEMENTED_PREFIX;

/// Generator output: the regenerated runner file and the write-once step
/// stub template.
#[derive(Debug)]
pub struct GeneratedSwift {
    pub runner: String,
    pub steps_template: String,
}

/// Resolve the test-target source directory (`[verify.swift]`):
/// `package-dir` (default `cwd`, default root) + `swift-tests-dir`
/// (default: the single directory under `<package-dir>/Tests/`).
///
/// # Errors
/// [`GenError::SwiftTestsDir`] when auto-detection finds no unambiguous
/// test target.
pub fn resolve_tests_dir(root: &Path, section: Option<&VerifyStack>) -> Result<PathBuf, GenError> {
    let package = package_dir(root, section);
    if let Some(rel) = section.and_then(|s| s.swift_tests_dir.as_deref()) {
        return Ok(package.join(rel));
    }
    let tests_root = package.join("Tests");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&tests_root)
        .map_err(|e| GenError::SwiftTestsDir {
            detail: format!("cannot read {} ({e})", tests_root.display()),
        })?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    match dirs.len() {
        1 => Ok(dirs.remove(0)),
        0 => Err(GenError::SwiftTestsDir {
            detail: format!("{} contains no test-target directory", tests_root.display()),
        }),
        n => Err(GenError::SwiftTestsDir {
            detail: format!(
                "{} contains {n} directories — ambiguous test target",
                tests_root.display()
            ),
        }),
    }
}

/// The `SwiftPM` package root for the swift stack.
#[must_use]
pub fn package_dir(root: &Path, section: Option<&VerifyStack>) -> PathBuf {
    section
        .and_then(|s| s.package_dir.as_deref().or(s.cwd.as_deref()))
        .map_or_else(|| root.to_path_buf(), |p| root.join(p))
}

/// The `@Suite` struct name for a feature.
///
/// `PascalCase` words + `Feature` (`Todo management` →
/// `TodoManagementFeature`), underscore-prefixed if it would start with a
/// digit. Deterministic — the verify adapter rebuilds it for the ADR-001
/// `--filter` recipe.
#[must_use]
pub fn suite_name(feature_name: &str) -> String {
    let mut pascal = String::new();
    for word in feature_name.split(|c: char| !c.is_alphanumeric()) {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            pascal.extend(first.to_uppercase());
            pascal.extend(chars);
        }
    }
    let name = format!("{pascal}Feature");
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{name}")
    } else {
        name
    }
}

/// The Swift Testing signature of a scenario's generated `@Test` function.
///
/// The mandatory suffix of the xcodebuild `-only-testing:` identifier
/// (verified empirically: the selector silently matches 0 tests without
/// it). Empty for plain scenarios; the argument-label form
/// (`"quantity:reason:"`) for outlines with ≤2 Examples columns
/// (destructured parameters); `"_:"` for 3+ columns (one `row` tuple —
/// see [`generate`]). `None` when no such scenario exists in the feature.
///
/// # Errors
/// [`GenError::MixedExampleHeaders`] when the outline's Examples tables
/// disagree on headers.
pub fn test_signature(
    feature: &gherkin::Feature,
    scenario_name: &str,
) -> Result<Option<String>, GenError> {
    let scenario = feature
        .scenarios
        .iter()
        .chain(feature.rules.iter().flat_map(|r| r.scenarios.iter()))
        .find(|s| s.name == scenario_name);
    let Some(scenario) = scenario else {
        return Ok(None);
    };
    let signature = example_table(scenario)?.map_or_else(String::new, |table| {
        let params = typed_params(&table);
        if params.len() <= 2 {
            params.iter().fold(String::new(), |mut out, p| {
                out.push_str(&param_name(&p.header));
                out.push(':');
                out
            })
        } else {
            "_:".to_owned()
        }
    });
    Ok(Some(signature))
}

/// Generate the runner file and step stub template for a linted feature.
///
/// # Errors
/// [`GenError::MixedExampleHeaders`] when an outline's Examples tables
/// disagree on headers.
pub fn generate(feature: &gherkin::Feature) -> Result<GeneratedSwift, GenError> {
    let mut registry = StepRegistry::default();
    let mut bodies = String::new();
    let mut tags: Vec<String> = Vec::new();

    let scenarios = feature
        .scenarios
        .iter()
        .chain(feature.rules.iter().flat_map(|r| r.scenarios.iter()));
    for scenario in scenarios {
        let table = example_table(scenario)?;
        for tag in &scenario.tags {
            let ident = tag_identifier(tag);
            if !tags.contains(&ident) {
                tags.push(ident);
            }
        }
        write_scenario(&mut bodies, scenario, table.as_ref(), &mut registry);
    }

    let mut runner = String::from(HEADER);
    runner.push_str("import Testing\n");
    if !tags.is_empty() {
        runner.push_str("\nextension Tag {\n");
        for tag in &tags {
            let _ = writeln!(runner, "    @Tag static var {tag}: Self");
        }
        runner.push_str("}\n");
    }
    let _ = write!(
        runner,
        "\n@Suite(\"Feature: {}\")\nstruct {} {{\n{bodies}}}\n",
        swift_string(&feature.name),
        suite_name(&feature.name),
    );

    Ok(GeneratedSwift {
        runner,
        steps_template: steps_template(&registry),
    })
}

const HEADER: &str = "\
// GENERATED by craftsman spec gen — do not edit.
// Fully regenerated from SPEC.md on every run (this file is the CLI's, per
// the single-writer rule). Step implementations live in your own
// Steps.swift: copy Steps.swift.template next to this file, rename it, and
// implement the SpecSteps methods — craftsman never touches those files.

";

fn write_scenario(
    out: &mut String,
    scenario: &gherkin::Scenario,
    table: Option<&ExampleTable>,
    registry: &mut StepRegistry,
) {
    let headers = table.map(typed_params).unwrap_or_default();
    let calls: Vec<super::StepCall> = scenario
        .steps
        .iter()
        .map(|s| registry.call(&s.keyword, &s.value, &headers))
        .collect();

    // @Test trait list: tags, then arguments for outlines.
    let tag_list = scenario
        .tags
        .iter()
        .map(|t| format!(".{}", tag_identifier(t)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut attr = String::from("    @Test");
    let name = &scenario.name;

    match table {
        None => {
            if !tag_list.is_empty() {
                let _ = write!(attr, "(.tags({tag_list}))");
            }
            let _ = writeln!(out, "{attr}\n    func `{name}`() throws {{");
            write_step_calls(out, &calls, None);
        }
        Some(table) => {
            let names: Vec<String> = headers.iter().map(|p| param_name(&p.header)).collect();
            attr.push('(');
            if !tag_list.is_empty() {
                let _ = write!(attr, ".tags({tag_list}), ");
            }
            attr.push_str("arguments: [\n");
            for row in &table.rows {
                let values: Vec<String> = row
                    .iter()
                    .enumerate()
                    .map(|(i, v)| swift_value(v, headers.get(i).is_some_and(|p| p.is_int)))
                    .collect();
                let tuple = if names.len() == 1 {
                    values.join(", ")
                } else {
                    let labeled: Vec<String> = names
                        .iter()
                        .zip(&values)
                        .map(|(p, v)| format!("{p}: {v}"))
                        .collect();
                    format!("({})", labeled.join(", "))
                };
                let _ = writeln!(attr, "        {tuple},");
            }
            attr.push_str("    ])");

            // ≤2 columns destructure into typed parameters (spike-proven);
            // 3+ arrive as one labeled tuple (Swift Testing destructures
            // two-element tuples only).
            let fields: Vec<String> = names
                .iter()
                .zip(&headers)
                .map(|(n, p)| format!("{n}: {}", swift_type(p.is_int)))
                .collect();
            let signature: String = if names.len() <= 2 {
                fields.join(", ")
            } else {
                format!("_ row: ({})", fields.join(", "))
            };
            let _ = writeln!(out, "{attr}\n    func `{name}`({signature}) throws {{");
            let row_access = (names.len() > 2).then_some("row.");
            write_step_calls(out, &calls, row_access);
        }
    }
    out.push_str("    }\n\n");
}

/// The body: one `try steps.step_…(…)` per step, on a shared `SpecSteps`
/// value (`var`: stubs are `mutating`, real steps may hold state).
fn write_step_calls(out: &mut String, calls: &[super::StepCall], row_access: Option<&str>) {
    if calls.is_empty() {
        out.push_str("        // SPEC.md lists no steps for this scenario.\n");
        return;
    }
    out.push_str("        var steps = SpecSteps()\n");
    for call in calls {
        let args: Vec<String> = call
            .params
            .iter()
            .map(|p| {
                let name = param_name(&p.header);
                row_access.map_or_else(|| name.clone(), |prefix| format!("{prefix}{name}"))
            })
            .collect();
        let _ = writeln!(out, "        try steps.{}({})", call.name, args.join(", "));
    }
}

fn steps_template(registry: &StepRegistry) -> String {
    let mut out = String::from(
        "\
// Step implementations for SPEC.md — written ONCE by `craftsman spec gen`
// and never overwritten (this file is yours from now on). Copy it next to
// the generated SpecScenarios.swift as `Steps.swift`, then implement each
// method. Unimplemented steps keep the #expect stub below; its
// \"step not implemented:\" message is how `craftsman verify` reports the
// scenario as Undefined rather than Failed.

import Testing

struct SpecSteps {
",
    );
    for f in registry.fns() {
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("_ {}: {}", param_name(&p.header), swift_type(p.is_int)))
            .collect();
        let _ = write!(
            out,
            "    // {}\n    mutating func {}({}) throws {{\n        \
             #expect(Bool(false), \"{NOT_IMPLEMENTED_PREFIX}{}\")\n    }}\n\n",
            f.display,
            f.name,
            params.join(", "),
            swift_string(&f.display),
        );
    }
    out.push_str("}\n");
    out
}

/// A valid Swift parameter identifier from an Examples header.
fn param_name(header: &str) -> String {
    let s = slug(header);
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{s}")
    } else {
        s
    }
}

/// A valid Swift identifier for a `Tag` from a Gherkin tag.
fn tag_identifier(tag: &str) -> String {
    let mut out = String::with_capacity(tag.len());
    for c in tag.chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{out}")
    } else {
        out
    }
}

/// Escape for a Swift string literal (`\`, `"`; `\(` interpolation is
/// covered by the backslash escape).
fn swift_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// An Examples cell as Swift source: bare integer or quoted string.
fn swift_value(value: &str, is_int: bool) -> String {
    if is_int {
        value.trim().to_owned()
    } else {
        format!("\"{}\"", swift_string(value))
    }
}

const fn swift_type(is_int: bool) -> &'static str {
    if is_int { "Int" } else { "String" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn todo_feature() -> gherkin::Feature {
        // The S1 spike's todo.feature, minus the @batchN tags spec lint
        // now bans (ADR-001 predates the batch-tag rule).
        let text = "\
Feature: Todo management

  @todo
  Scenario: Adding a todo shows it in the list
    Given an empty todo list
    When I add a todo \"Buy milk\"
    Then the list contains \"Buy milk\"

  @cart
  Scenario Outline: Rejecting an invalid quantity keeps the cart unchanged
    Given a cart with quantity 1
    When I set the quantity to <quantity>
    Then the update is rejected as \"<reason>\"

    Examples:
      | quantity | reason     |
      | 0        | zero       |
      | -3       | negative   |
      | 1000     | over-limit |
";
        gherkin::Feature::parse(text, gherkin::GherkinEnv::default()).expect("fixture parses")
    }

    #[test]
    fn suite_names_match_the_spike() {
        assert_eq!(suite_name("Todo management"), "TodoManagementFeature");
        assert_eq!(suite_name("Craftsman CLI core"), "CraftsmanCLICoreFeature");
        assert_eq!(suite_name("2048 game"), "_2048GameFeature");
    }

    #[test]
    fn runner_matches_the_spike_shape() {
        let out = generate(&todo_feature()).expect("generates");
        let r = &out.runner;
        assert!(r.starts_with("// GENERATED by craftsman spec gen"), "{r}");
        assert!(r.contains("@Suite(\"Feature: Todo management\")"), "{r}");
        assert!(r.contains("struct TodoManagementFeature {"), "{r}");
        assert!(
            r.contains("func `Adding a todo shows it in the list`() throws {"),
            "{r}"
        );
        assert!(r.contains("@Test(.tags(.todo))"), "{r}");
        assert!(r.contains("@Tag static var cart: Self"), "{r}");
        // The outline: labeled 2-tuples, destructured typed parameters.
        assert!(r.contains("(quantity: 0, reason: \"zero\"),"), "{r}");
        assert!(
            r.contains("(quantity: 1000, reason: \"over-limit\"),"),
            "{r}"
        );
        assert!(
            r.contains(
                "func `Rejecting an invalid quantity keeps the cart unchanged`\
                 (quantity: Int, reason: String) throws {"
            ),
            "{r}"
        );
        assert!(
            r.contains("try steps.step_i_set_the_quantity_to(quantity)"),
            "{r}"
        );
    }

    #[test]
    fn test_signatures_match_the_generated_functions() {
        let f = todo_feature();
        assert_eq!(
            test_signature(&f, "Adding a todo shows it in the list").expect("no header clash"),
            Some(String::new()),
            "plain scenarios have the empty signature"
        );
        assert_eq!(
            test_signature(&f, "Rejecting an invalid quantity keeps the cart unchanged")
                .expect("no header clash"),
            Some("quantity:reason:".to_owned()),
            "≤2 columns destructure into labeled parameters"
        );
        assert_eq!(
            test_signature(&f, "No such scenario").expect("no header clash"),
            None
        );
    }

    #[test]
    fn steps_template_stubs_every_unique_step_with_the_marker() {
        let out = generate(&todo_feature()).expect("generates");
        let t = &out.steps_template;
        assert!(t.contains("struct SpecSteps {"), "{t}");
        assert!(
            t.contains("mutating func step_an_empty_todo_list() throws {"),
            "{t}"
        );
        assert!(
            t.contains("mutating func step_i_set_the_quantity_to(_ quantity: Int) throws {"),
            "{t}"
        );
        assert!(
            t.contains("#expect(Bool(false), \"step not implemented: Given an empty todo list\")"),
            "{t}"
        );
        // 6 unique steps → 6 stubs.
        assert_eq!(t.matches("mutating func step_").count(), 6, "{t}");
    }
}

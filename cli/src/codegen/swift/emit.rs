//! Swift source emission: the generated `@Test` scenario functions, the
//! write-once step-stub template, and the identifier/string escaping
//! helpers they share.

use std::fmt::Write as _;

use crate::codegen::steps::{ExampleTable, StepCall, StepRegistry, slug, typed_params};
use crate::verify::normalize::NOT_IMPLEMENTED_PREFIX;

pub(super) fn write_scenario(
    out: &mut String,
    scenario: &gherkin::Scenario,
    table: Option<&ExampleTable>,
    registry: &mut StepRegistry,
) {
    let headers = table.map(typed_params).unwrap_or_default();
    let calls: Vec<StepCall> = scenario
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
fn write_step_calls(out: &mut String, calls: &[StepCall], row_access: Option<&str>) {
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

pub(super) fn steps_template(registry: &StepRegistry) -> String {
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
pub(super) fn param_name(header: &str) -> String {
    let s = slug(header);
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{s}")
    } else {
        s
    }
}

/// A valid Swift identifier for a `Tag` from a Gherkin tag.
pub(super) fn tag_identifier(tag: &str) -> String {
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
pub(super) fn swift_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// An Examples cell as Swift source: bare integer or quoted string.
pub(super) fn swift_value(value: &str, is_int: bool) -> String {
    if is_int {
        value.trim().to_owned()
    } else {
        format!("\"{}\"", swift_string(value))
    }
}

const fn swift_type(is_int: bool) -> &'static str {
    if is_int { "Int" } else { "String" }
}

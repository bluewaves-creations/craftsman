//! Shared step machinery for the code-gen stacks: parameter extraction
//! from step text, the step registry with de-collision, and Examples-table
//! typing — everything `swift.rs` and `bash.rs` both consume.

use super::GenError;

/// One outline parameter a step takes: the Examples header and whether its
/// column is integer-typed (Swift `Int` vs `String`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub header: String,
    pub is_int: bool,
}

/// A step reference inside one scenario: which unique step function it
/// calls and with which outline parameters (empty outside outlines).
#[derive(Debug, Clone)]
pub struct StepCall {
    /// Final function name, `step_<slug>` (de-collided).
    pub name: String,
    /// Outline parameters used by this step, in order of appearance.
    pub params: Vec<Param>,
}

/// One unique step function to stub.
#[derive(Debug, Clone)]
pub struct StepFn {
    pub name: String,
    /// Human text for the not-implemented marker: `<keyword> <value>`.
    pub display: String,
    /// Outline parameters this step takes, in order.
    pub params: Vec<Param>,
}

/// Lowercase snake slug: alphanumerics kept, everything else collapses to
/// single underscores. Never empty (falls back to `step`).
pub fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_underscore = true;
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            last_underscore = false;
        } else if !last_underscore {
            out.push('_');
            last_underscore = true;
        }
    }
    let trimmed = out.trim_end_matches('_');
    if trimmed.is_empty() {
        "step".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// The `<placeholder>` parameters present in a step's text that are outline
/// headers, in order of appearance.
fn step_params(value: &str, headers: &[Param]) -> Vec<Param> {
    let mut params: Vec<(usize, Param)> = Vec::new();
    for param in headers {
        if let Some(pos) = value.find(&format!("<{}>", param.header))
            && !params.iter().any(|(_, p)| p.header == param.header)
        {
            params.push((pos, param.clone()));
        }
    }
    params.sort_by_key(|(pos, _)| *pos);
    params.into_iter().map(|(_, p)| p).collect()
}

/// Step text with outline placeholders removed, for slugging — so every
/// Examples row calls the same function.
fn strip_placeholders(value: &str, headers: &[Param]) -> String {
    let mut out = value.to_owned();
    for param in headers {
        out = out.replace(&format!("<{}>", param.header), " ");
    }
    out
}

/// Per-feature step registry: assigns each unique step (by slug + params)
/// a stable, collision-free function name.
#[derive(Debug, Default)]
pub struct StepRegistry {
    fns: Vec<StepFn>,
}

impl StepRegistry {
    /// Register a step occurrence, returning its call site.
    pub fn call(&mut self, keyword: &str, value: &str, headers: &[Param]) -> StepCall {
        let params = step_params(value, headers);
        let base = format!("step_{}", slug(&strip_placeholders(value, headers)));
        let display = format!("{} {}", keyword.trim(), value);

        // Same slug + same params = same step function (first display wins).
        if let Some(existing) = self
            .fns
            .iter()
            .find(|f| f.params == params && (f.name == base || is_decollision_of(&f.name, &base)))
        {
            return StepCall {
                name: existing.name.clone(),
                params,
            };
        }
        // De-collide against same-named functions with different params.
        let mut name = base.clone();
        let mut n = 1;
        while self.fns.iter().any(|f| f.name == name) {
            n += 1;
            name = format!("{base}_{n}");
        }
        self.fns.push(StepFn {
            name: name.clone(),
            display,
            params: params.clone(),
        });
        StepCall { name, params }
    }

    pub fn fns(&self) -> &[StepFn] {
        &self.fns
    }
}

/// Whether `name` is `base` with a `_<n>` de-collision suffix.
fn is_decollision_of(name: &str, base: &str) -> bool {
    name.strip_prefix(base)
        .and_then(|rest| rest.strip_prefix('_'))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// A scenario outline's Examples: shared headers and every row.
#[derive(Debug, Clone)]
pub struct ExampleTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Collect a scenario's Examples rows across its tables, requiring
/// identical headers.
///
/// Returns `None` for a plain scenario (no Examples).
pub fn example_table(scenario: &gherkin::Scenario) -> Result<Option<ExampleTable>, GenError> {
    let mut merged: Option<ExampleTable> = None;
    for examples in &scenario.examples {
        let Some(table) = &examples.table else {
            continue;
        };
        let Some((headers, rows)) = table.rows.split_first() else {
            continue;
        };
        match &mut merged {
            None => {
                merged = Some(ExampleTable {
                    headers: headers.clone(),
                    rows: rows.to_vec(),
                });
            }
            Some(t) if t.headers == *headers => t.rows.extend(rows.iter().cloned()),
            Some(t) => {
                return Err(GenError::MixedExampleHeaders {
                    scenario: scenario.name.clone(),
                    first: t.headers.clone(),
                    second: headers.clone(),
                });
            }
        }
    }
    Ok(merged)
}

/// Whether every value of column `i` parses as an integer (typed columns
/// become `Int` in Swift; everything else stays a string).
pub fn column_is_int(table: &ExampleTable, i: usize) -> bool {
    !table.rows.is_empty()
        && table
            .rows
            .iter()
            .all(|row| row.get(i).is_some_and(|v| v.trim().parse::<i64>().is_ok()))
}

/// A table's headers as typed [`Param`]s.
pub fn typed_params(table: &ExampleTable) -> Vec<Param> {
    table
        .headers
        .iter()
        .enumerate()
        .map(|(i, h)| Param {
            header: h.clone(),
            is_int: column_is_int(table, i),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_collapses_and_lowercases() {
        assert_eq!(slug("An empty todo list"), "an_empty_todo_list");
        assert_eq!(slug("I add a todo \"Buy milk\""), "i_add_a_todo_buy_milk");
        assert_eq!(slug("Café — fermé!"), "café_fermé");
        assert_eq!(slug("   "), "step");
    }

    fn param(header: &str) -> Param {
        Param {
            header: header.to_owned(),
            is_int: false,
        }
    }

    #[test]
    fn registry_reuses_identical_steps_and_decollides_conflicts() {
        let mut reg = StepRegistry::default();
        let headers = vec![param("quantity")];
        let a = reg.call("Given", "an empty list", &[]);
        let b = reg.call("When", "an empty list", &[]);
        assert_eq!(a.name, b.name, "same text = same step function");
        // Same slug, different params → a distinct de-collided function.
        let c = reg.call("When", "an empty list <quantity>", &headers);
        assert_eq!(c.name, "step_an_empty_list_2");
        assert_eq!(c.params, headers);
        assert_eq!(reg.fns().len(), 2);
    }

    #[test]
    fn step_params_follow_appearance_order() {
        let headers = vec![param("reason"), param("quantity")];
        assert_eq!(
            step_params("sets <quantity> because <reason>", &headers)
                .into_iter()
                .map(|p| p.header)
                .collect::<Vec<_>>(),
            vec!["quantity".to_owned(), "reason".to_owned()]
        );
    }
}

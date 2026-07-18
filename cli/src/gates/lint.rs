//! `craftsman lint` — the lint gate over the configured stacks.
//!
//! Tools per stack (all declarative, see `adapter::TOOLS`): rust →
//! `cargo fmt --check` + `cargo clippy` (moved here from ledger.rs, which
//! hard-coded the pair in Batch 3); python → uvx ruff check + format
//! --check; typescript → bunx biome check; swift → swiftlint; bash →
//! shellcheck over tracked `.sh`/`.bash` files.
//!
//! `--changed` narrowing per adapter: file lists are passed to the tools
//! that accept them (ruff, biome, swiftlint, shellcheck); cargo tools run
//! project-wide and their findings are filtered to the changed set
//! afterwards (cargo has no per-file mode — a full run then filter is the
//! honest equivalent).

use std::path::Path;

use super::adapter::{self, BaselineKind, GateTool};
use super::{Finding, GateError, GateOutcome, baseline, exec, tail, tools};
use crate::config::{Config, GateMode};

/// Run the lint gate.
///
/// `changed`: `None` = full run; `Some(files)` = root-relative changed
/// set. `mode` is the enforcement mode to apply (callers pass the
/// configured mode, or strict for direct invocation of an off gate).
///
/// # Errors
/// [`GateError`] when a tool cannot be resolved, spawned, or parsed —
/// tool failure is never a green gate.
pub fn run(
    root: &Path,
    config: &Config,
    changed: Option<&[String]>,
    mode: GateMode,
) -> Result<GateOutcome, GateError> {
    let mut findings: Vec<Finding> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut tools_ran: Vec<&'static str> = Vec::new();

    for stack in &config.project.stacks {
        let cwd = config
            .verify
            .stack(stack)
            .and_then(|s| s.cwd.as_deref())
            .map(|c| c.trim_end_matches('/').to_owned());
        let stack_changed = changed.map(|files| scoped(files, cwd.as_deref()));
        if let Some(scoped_files) = &stack_changed
            && scoped_files.is_empty()
        {
            notes.push(format!("stack {stack}: no changed files — tools skipped"));
            continue;
        }
        for tool in adapter::TOOLS
            .iter()
            .filter(|t| t.gate == "lint" && t.stack == *stack)
        {
            let run = run_tool(
                root,
                config,
                tool,
                cwd.as_deref(),
                stack_changed.as_deref(),
                mode,
            )?;
            match run {
                ToolRun::Ran(mut tool_findings) => {
                    if let Some(files) = changed
                        && !tool.accepts_files
                    {
                        // Full run, findings filtered (fmt/clippy).
                        tool_findings.retain(|f| files.iter().any(|c| c == &f.file));
                    }
                    findings.append(&mut tool_findings);
                    tools_ran.push(tool.name);
                }
                ToolRun::Skipped(reason) => notes.push(reason),
            }
        }
    }

    finish(root, "lint", findings, notes, tools_ran, changed, mode)
}

/// Shared gate epilogue: apply the mode (baseline vs strict) and assemble
/// the outcome. Security reuses this.
pub(crate) fn finish(
    root: &Path,
    gate: &'static str,
    findings: Vec<Finding>,
    mut notes: Vec<String>,
    tools_ran: Vec<&'static str>,
    changed: Option<&[String]>,
    mode: GateMode,
) -> Result<GateOutcome, GateError> {
    let (blocking, baselined, ratchet) = match mode {
        GateMode::Baseline => {
            let snapshot_tools: Vec<&'static str> = tools_ran
                .iter()
                .copied()
                .filter(|name| {
                    adapter::tool(name).is_some_and(|t| t.baseline == BaselineKind::Snapshot)
                })
                .collect();
            let (snapshot, native): (Vec<Finding>, Vec<Finding>) = findings
                .clone()
                .into_iter()
                .partition(|f| snapshot_tools.contains(&f.tool));
            let applied =
                baseline::apply(root, gate, snapshot, &snapshot_tools, changed.is_none())?;
            // Native-baseline tools already diffed tool-side: everything
            // they still report is new.
            let mut blocking = applied.new_findings;
            blocking.extend(native);
            (blocking, applied.baselined, applied.ratchet)
        }
        GateMode::Strict | GateMode::Off => (findings.clone(), 0, None),
    };
    if mode == GateMode::Off {
        notes.push(format!(
            "gate {gate} is off in craftsman.toml — this direct run enforced strict"
        ));
    }
    Ok(GateOutcome {
        gate,
        mode: if mode == GateMode::Off {
            GateMode::Strict
        } else {
            mode
        },
        findings,
        blocking,
        baselined,
        ratchet,
        notes,
        tools_ran,
    })
}

/// What one tool invocation produced.
enum ToolRun {
    Ran(Vec<Finding>),
    Skipped(String),
}

fn run_tool(
    root: &Path,
    config: &Config,
    tool: &'static GateTool,
    cwd: Option<&str>,
    stack_changed: Option<&[String]>,
    mode: GateMode,
) -> Result<ToolRun, GateError> {
    let version = pinned_version(config, tool);
    let resolved = tools::resolve(tool, &version)?;
    let dir = cwd.map_or_else(|| root.to_path_buf(), |c| root.join(c));

    let mut argv = resolved.argv.clone();
    argv.extend(tool.base_args.iter().map(|s| (*s).to_owned()));

    // Targets: explicit file lists where the tool takes them.
    match tool.name {
        "shellcheck" => {
            let mut files: Vec<String> = super::git(root, &["ls-files", "*.sh", "*.bash"])?
                .lines()
                .map(str::to_owned)
                .collect();
            if let Some(changed) = stack_changed {
                files.retain(|f| changed.contains(f));
            }
            if files.is_empty() {
                return Ok(ToolRun::Skipped(
                    "shellcheck: no tracked shell files — skipped".to_owned(),
                ));
            }
            argv.extend(files);
        }
        "swiftlint" => {
            if mode == GateMode::Baseline {
                let native = baseline::path(root, "swiftlint");
                if native.is_file() {
                    argv.push("--baseline".to_owned());
                    argv.push(native.to_string_lossy().into_owned());
                }
            }
            if let Some(changed) = stack_changed {
                let swift: Vec<String> = changed
                    .iter()
                    .filter(|f| {
                        Path::new(f)
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("swift"))
                    })
                    .map(|f| root.join(f).to_string_lossy().into_owned())
                    .collect();
                if swift.is_empty() {
                    return Ok(ToolRun::Skipped(
                        "swiftlint: no changed swift files — skipped".to_owned(),
                    ));
                }
                argv.extend(swift);
            }
        }
        "ruff" | "ruff-format" | "biome" => {
            if let Some(changed) = stack_changed {
                let dir_relative: Vec<String> = changed
                    .iter()
                    .filter(|f| relevant_for(tool.name, f))
                    .map(|f| strip_cwd(f, cwd))
                    .collect();
                if dir_relative.is_empty() {
                    return Ok(ToolRun::Skipped(format!(
                        "{}: no changed files for it — skipped",
                        tool.name
                    )));
                }
                argv.extend(dir_relative);
            } else {
                argv.push(".".to_owned());
            }
        }
        _ => {}
    }

    eprintln!("gate lint: {} ({}) …", tool.name, resolved.via);
    let output = exec(&argv, &dir, &[])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let code = output.status.code().unwrap_or(-1);

    let findings = adapter::parse(tool, &stdout, &stderr)?;
    let ran_clean = tool.success_codes.contains(&code);
    if !ran_clean && findings.is_empty() {
        return Err(GateError::ToolFailed {
            tool: format!("{} ({})", tool.name, argv.join(" ")),
            code: code.to_string(),
            output: tail(&format!("{stdout}{stderr}"), 30),
        });
    }

    Ok(ToolRun::Ran(normalize_paths(findings, root, cwd)))
}

/// The pinned version for a tool: `[gates.tools]` overrides the adapter
/// default (ruff-format shares ruff's pin).
fn pinned_version(config: &Config, tool: &GateTool) -> String {
    let key = if tool.name == "ruff-format" {
        "ruff"
    } else {
        tool.name
    };
    config
        .gates
        .tools
        .get(key)
        .cloned()
        .unwrap_or_else(|| tool.default_version.to_owned())
}

/// Changed files scoped to a stack root, kept root-relative.
fn scoped(files: &[String], cwd: Option<&str>) -> Vec<String> {
    cwd.map_or_else(
        || files.to_vec(),
        |prefix| {
            let with_slash = format!("{prefix}/");
            files
                .iter()
                .filter(|f| f.starts_with(&with_slash))
                .cloned()
                .collect()
        },
    )
}

fn relevant_for(tool: &str, file: &str) -> bool {
    match tool {
        "ruff" | "ruff-format" => Path::new(file)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("py")),
        "biome" => [".ts", ".tsx", ".js", ".jsx", ".json", ".css"]
            .iter()
            .any(|ext| file.ends_with(ext)),
        _ => true,
    }
}

/// A root-relative path re-expressed relative to the stack cwd (tools run
/// there).
fn strip_cwd(file: &str, cwd: Option<&str>) -> String {
    cwd.and_then(|c| file.strip_prefix(&format!("{c}/")))
        .unwrap_or(file)
        .to_owned()
}

/// Normalize finding paths to root-relative: absolute paths lose the root
/// prefix; cwd-relative paths gain the cwd prefix.
fn normalize_paths(findings: Vec<Finding>, root: &Path, cwd: Option<&str>) -> Vec<Finding> {
    let root_str = format!("{}/", root.display());
    findings
        .into_iter()
        .map(|mut f| {
            if let Some(stripped) = f.file.strip_prefix(&root_str) {
                f.file = stripped.to_owned();
            } else if !f.file.starts_with('/')
                && let Some(prefix) = cwd
            {
                f.file = format!("{prefix}/{}", f.file);
            }
            f
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_keeps_only_the_stack_prefix() {
        let files = vec!["cli/src/a.rs".to_owned(), "docs/x.md".to_owned()];
        assert_eq!(scoped(&files, Some("cli")), vec!["cli/src/a.rs".to_owned()]);
        assert_eq!(scoped(&files, None).len(), 2);
    }

    #[test]
    fn paths_normalize_to_root_relative() {
        let f = |file: &str| Finding {
            gate: "lint",
            tool: "fmt",
            rule: "r".to_owned(),
            file: file.to_owned(),
            line: None,
            message: "m".to_owned(),
            severity: super::super::Severity::Low,
        };
        let out = normalize_paths(
            vec![f("/repo/cli/src/a.rs"), f("src/b.rs")],
            Path::new("/repo"),
            Some("cli"),
        );
        assert_eq!(out[0].file, "cli/src/a.rs");
        assert_eq!(out[1].file, "cli/src/b.rs");
    }

    #[test]
    fn ruff_format_shares_the_ruff_pin() {
        let config = Config::from_toml(
            "[project]\nname = \"x\"\nstacks = [\"python\"]\n[gates.tools]\nruff = \"9.9.9\"\n",
            Path::new("craftsman.toml"),
        )
        .expect("parses");
        let rf = adapter::tool("ruff-format").expect("in table");
        assert_eq!(pinned_version(&config, rf), "9.9.9");
    }
}

//! Self-hosting acceptance harness: runs the repo-root SPEC.md with
//! cucumber-rs, driving the compiled `craftsman` binary against disposable
//! fixture projects in temp directories.
//!
//! ADR-003 convention (which `craftsman verify` relies on): when the
//! `CRAFTSMAN_JSON` environment variable is set, the harness writes
//! cucumber-json results there; otherwise it runs with the default writer
//! and a non-zero exit on failure (`cargo test --test spec`).

#![expect(
    clippy::needless_pass_by_value,
    reason = "cucumber's step macros pass owned, FromStr-extracted parameters"
)]
#![expect(
    clippy::needless_pass_by_ref_mut,
    reason = "cucumber's step macros require `&mut World` as the first argument"
)]

mod adopt_steps;
mod codegen_steps;
mod docs_steps;
mod engine_steps;
mod gates_steps;
mod impact_steps;
mod import_steps;
mod ledger_steps;
mod mutate_steps;
mod probes;
mod project_steps;
mod repo_steps;
mod runtime_steps;
mod security_steps;
mod setup_steps;
mod update_steps;

use std::path::PathBuf;
use std::process::{Command, Output};

use cucumber::World as _;

const MINIMAL_CONFIG: &str = "[project]\nname = \"fixture\"\nstacks = [\"rust\"]\n";

#[derive(Debug, Default, cucumber::World)]
pub struct CliWorld {
    pub dir: Option<tempfile::TempDir>,
    /// A cached scaffolded fixture at a stable path (its compiled `target/`
    /// survives across runs, like doctor's) instead of a throwaway tempdir.
    fixed_dir: Option<PathBuf>,
    /// A sandboxed `$HOME` for the setup scenarios.
    home: Option<tempfile::TempDir>,
    /// Extra environment for the next craftsman invocation (e.g. a dead
    /// release-channel endpoint for the unreachable-update scenario).
    env: Vec<(String, String)>,
    /// Exit code of an earlier invocation in a multi-command When step.
    prev_exit: Option<i32>,
    /// A HEAD sha recorded by a Given, for unchanged-head assertions.
    remembered_head: Option<String>,
    output: Option<Output>,
}

impl CliWorld {
    /// The fixture project directory, created on first use.
    fn project_dir(&mut self) -> PathBuf {
        if let Some(fixed) = &self.fixed_dir {
            return fixed.clone();
        }
        if self.dir.is_none() {
            self.dir = Some(tempfile::tempdir().expect("create fixture tempdir"));
        }
        self.dir
            .as_ref()
            .expect("just created")
            .path()
            .to_path_buf()
    }

    fn write(&mut self, name: &str, content: &str) {
        let path = self.project_dir().join(name);
        std::fs::write(&path, content).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    }

    fn run_craftsman(&mut self, args: &[&str]) {
        let dir = self.project_dir();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_craftsman"));
        cmd.args(args).current_dir(&dir);
        if let Some(home) = &self.home {
            // A sandboxed home must really sandbox: without these removals
            // the machine's real receipt/config would leak into the run.
            cmd.env("HOME", home.path())
                .env_remove("XDG_CONFIG_HOME")
                .env_remove("AXOUPDATER_CONFIG_PATH")
                .env_remove("CRAFTSMAN_INSTALLER_GITHUB_BASE_URL");
        }
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        self.output = Some(cmd.output().expect("spawn craftsman"));
    }

    const fn output(&self) -> &Output {
        self.output
            .as_ref()
            .expect("a When step must run craftsman first")
    }

    fn combined_output(&self) -> String {
        let o = self.output();
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        )
    }
}

/// `@requires-*` capability gates. Network is an explicit grant
/// (`CRAFTSMAN_LIVE=1`, a policy decision); toolchain tags (`swift`,
/// `xcode`, `chromium`) probe the machine once and exclude the scenario
/// when the capability is absent — the loud-skip philosophy of the cargo
/// integration tests, expressed as tags. Excluded scenarios emit no
/// result at all, so `spec status` reports them as unknown — visible,
/// never silently green.
///
/// TRAP (cucumber 0.23, cucumber.rs `filter_run`): when a `--name` regex
/// is present in the CLI opts, cucumber consults ONLY the regex and never
/// calls the programmatic filter. The gate must therefore compose the
/// regex itself: `main` takes `re_filter` out of the parsed opts and this
/// filter applies both, so a name-filtered run can never force a gated
/// scenario live.
fn scenario_gate(s: &cucumber::gherkin::Scenario) -> bool {
    s.tags.iter().all(|t| match t.as_str() {
        "requires-network" => std::env::var("CRAFTSMAN_LIVE").is_ok_and(|v| v == "1"),
        "requires-swift" => probes::swift(),
        "requires-xcode" => probes::xcode(),
        "requires-chromium" => probes::chromium(),
        _ => true,
    })
}

#[tokio::main]
async fn main() {
    // Repo-root SPEC.md, one directory above this cargo package. The
    // cucumber parser accepts a direct file path regardless of extension
    // and falls back to CARGO_MANIFEST_DIR-relative resolution.
    let spec = "../SPEC.md";
    if let Ok(path) = std::env::var("CRAFTSMAN_JSON") {
        // craftsman verify is driving: write cucumber-json where told.
        let file = std::fs::File::create(&path).unwrap_or_else(|e| panic!("create {path}: {e}"));
        let mut opts = cucumber::cli::Opts::<
            cucumber::parser::basic::Cli,
            cucumber::runner::basic::Cli,
            cucumber::cli::Empty,
        >::parsed();
        let name_re = opts.re_filter.take();
        CliWorld::cucumber()
            .with_writer(cucumber::writer::Json::new(file))
            .with_cli(opts)
            .filter_run(spec, move |_, _, s| {
                scenario_gate(s) && name_re.as_ref().is_none_or(|re| re.is_match(&s.name))
            })
            .await;
    } else {
        // Direct `cargo test --test spec`: human output, non-zero on red.
        let mut opts = cucumber::cli::Opts::<
            cucumber::parser::basic::Cli,
            cucumber::runner::basic::Cli,
            cucumber::writer::basic::Cli,
        >::parsed();
        let name_re = opts.re_filter.take();
        CliWorld::cucumber()
            .with_cli(opts)
            .filter_run_and_exit(spec, move |_, _, s| {
                scenario_gate(s) && name_re.as_ref().is_none_or(|re| re.is_match(&s.name))
            })
            .await;
    }
}

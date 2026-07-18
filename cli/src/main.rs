//! The Craftsman Dev CLI — mechanical verification for agentic development.
//!
//! Command layer only: clap parsing here, argument shapes and output
//! routing in `commands/*`, all logic in the library modules
//! (`thiserror`); the command layer is the sole `anyhow` consumer per
//! repo conventions.

mod commands;

use clap::{Parser, Subcommand};

use commands::{EXIT_ORCHESTRATOR_ERROR, bootstrap, docs, gate, ledger, session, spec, verify};

/// The Craftsman Dev CLI — mechanical verification for agentic development.
///
/// Exit codes: 0 pass · 1 verification failure · 2 usage error ·
/// 3 orchestrator error · 4 empty selection.
/// Every command supports --json (JSON to stdout, human progress to stderr).
#[derive(Parser)]
#[command(name = "craftsman", version = craftsman_version(), about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scaffold a new craftsman project — non-interactive: flags in,
    /// files out (the craftsman-init skill drives the interview).
    ///
    /// Writes craftsman.toml (verify + lint strict, security baseline),
    /// an AGENTS.md skeleton (headings + the Documentation Sources table
    /// header — content is the human's), a walking-skeleton SPEC.md,
    /// .craftsman/ dirs, .gitignore entries, a CLAUDE.md → AGENTS.md
    /// symlink, and harness hook templates (.claude/settings.json wired
    /// to `craftsman check-all --changed`; .cursor/hooks.json blocking on red gates at stop).
    ///
    /// Exit codes: 0 scaffolded · 2 usage error · 3 not a git repo,
    /// unknown stack, or existing files without --force (listed first —
    /// nothing is written while any conflict stands).
    Init(bootstrap::InitArgs),
    /// Bring a tree that arrived from elsewhere under craftsman (ADR-006):
    /// scaffold the contract non-destructively (existing files are kept,
    /// never overwritten), detect existing QA commands as [gates.qa]
    /// conversion candidates, and — with --audit — run every enabled gate
    /// strict and report the complete flaw inventory without recording a
    /// baseline. Debt disposal is explicit and human-gated.
    ///
    /// Exit codes: 0 imported (or audit reported) · 2 usage error ·
    /// 3 not a git repo, unknown stack, missing config for --audit, or a
    /// gate tool that cannot run.
    Import(bootstrap::ImportArgs),
    /// Brownfield adoption — the five-phase state machine (observe →
    /// ledger → hold-the-line → recover → steady-state), resumable via
    /// .craftsman/adoption.toml (CLI-written, committed).
    ///
    /// Phase 1 start writes the gates-off craftsman.toml + ADR-000
    /// template; phase 2 start records a baseline for every gate in
    /// baseline mode. Phases 0, 3, and 4 are skill-driven — the CLI only
    /// tracks state. Every transition records a timestamp and git HEAD.
    ///
    /// Exit codes: 0 recorded/reported · 2 usage error · 3 out-of-order
    /// phase, unknown phase, or no repo.
    Adopt(bootstrap::AdoptArgs),
    /// Install the six bundled craftsman-* skills: canonical copies into
    /// ~/.agents/skills/, then per-agent links via the adapter table
    /// (Claude Code symlinks; Codex/Cursor/Gemini/opencode/Goose/Pi read
    /// the canonical dir natively).
    ///
    /// Attribution-checked, never destructive: setup only replaces
    /// symlinks resolving into the canonical dir or trees it can prove it
    /// wrote (.craftsman-setup sentinel with the tree's sha256). Foreign
    /// content is reported and left; --force overrides, still listing.
    ///
    /// Exit codes: 0 done (refusals are report rows) · 2 usage error ·
    /// 3 no HOME / IO failure.
    Setup(bootstrap::SetupArgs),
    /// Update craftsman: refresh the installed skills from this binary's
    /// embedded payload, then self-update the binary to the latest release
    /// named by the cargo-dist install receipt
    /// (~/.config/craftsman/craftsman-receipt.json).
    ///
    /// No receipt (not installed from a release) reports the reinstall
    /// path — install.sh (GitHub Release) or `cargo install --path cli` —
    /// and exits 0. The one network-using command outside `docs sync`.
    ///
    /// Exit codes: 0 updated / already latest / no receipt ·
    /// 1 release channel unreachable or install failed · 3 environment
    /// error (no home, invalid running version).
    Update {
        /// Emit the report as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// SPEC.md engine: scenario inventory and authoring checks
    Spec {
        #[command(subcommand)]
        command: spec::SpecCommand,
    },
    /// Plan engine: keep the batch → scenario mapping honest
    Plan {
        #[command(subcommand)]
        command: spec::PlanCommand,
    },
    /// THE gate: run SPEC.md scenarios via the stack adapter.
    ///
    /// Exit codes: 0 all passed · 1 any failed/undefined/ambiguous ·
    /// 3 tool or config error · 4 selection matched no scenarios.
    Verify(verify::VerifyArgs),
    /// Structured ledger commit — the single writer of Verified-by.
    ///
    /// Commits exactly what is already staged (stage with `git add` first;
    /// exit 3 when nothing is staged) and only after the gates are green
    /// (exit 1 on a red gate, nothing committed). The Verified-by trailer
    /// is written by the CLI alone — there is no flag to set it. Configure
    /// an optional Co-Authored-By trailer via `[ledger] co-author` in
    /// craftsman.toml.
    Commit(ledger::CommitArgs),
    /// The lint gate: per-stack linters and formatters (rust: cargo fmt +
    /// clippy; python: ruff; typescript: biome; swift: swiftlint; bash:
    /// shellcheck), hermetically pinned via [gates.tools].
    ///
    /// Exit codes: 0 clean (or no new findings in baseline mode) · 1
    /// blocking findings · 3 tool failure (a broken tool is never green).
    Lint {
        /// Limit to files changed against HEAD (tools without file-list
        /// support run in full and their findings are filtered)
        #[arg(long)]
        changed: bool,
        /// Emit findings as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// The security gate: gitleaks (git history), semgrep (pinned
    /// ruleset, ERROR severity), osv-scanner (lockfiles) — in parallel.
    ///
    /// Findings at or above [gates] security-threshold (default: high)
    /// block. --changed never narrows the scan (standing risk); the
    /// check-all cache is the fast path.
    ///
    /// Exit codes: 0 clean (or no new findings in baseline mode) · 1
    /// blocking findings · 3 scanner failure (a broken scanner is never
    /// a green gate).
    Security {
        /// Accepted for gate-surface symmetry; the scan always runs full
        #[arg(long)]
        changed: bool,
        /// Emit findings as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// The arch gate: dependency-direction fitness rules from
    /// [arch] deny = ["A -> B", ...] — a file under path prefix A (relative
    /// to the stack root) importing anything under B is a violation.
    ///
    /// v1 is textual import extraction (rust `use crate::`, python
    /// import/from, ts relative imports, swift modules via Package.swift
    /// targets, bash source). Exit 3 when no rules are configured — an
    /// enabled gate with zero rules is never silent green.
    Arch {
        /// Accepted for gate-surface symmetry; dependency direction is a
        /// whole-graph property, the scan always runs full
        #[arg(long)]
        changed: bool,
        /// Emit findings as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// The health gate: function length, file length, complexity
    /// approximation, and duplicate blocks — craftsman's own
    /// deterministic metrics, thresholds in [health].
    ///
    /// Exit codes: 0 clean (or no new findings in baseline mode) · 1
    /// blocking findings.
    Health {
        /// Narrow reported findings to files changed against HEAD (the
        /// scan itself always covers the repo — duplication is
        /// cross-file)
        #[arg(long)]
        changed: bool,
        /// Emit findings as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// The mutate gate: diff-scoped mutation testing (rust:
    /// cargo-mutants --in-diff; python: mutmut on changed files;
    /// typescript: Stryker incremental). Score on changed code must reach
    /// [mutate] min-score (default 60); survived mutants are findings.
    ///
    /// Diff-scoped by design: full runs are slow, so --all demands
    /// --yes-slow — asking for --all alone is a usage error (exit 2,
    /// enforced by the argument parser). Swift/bash stacks are refused
    /// loudly (exit 3): no production-consensus tool exists.
    Mutate {
        /// Accepted for symmetry; mutation is diff-scoped by default
        #[arg(long)]
        changed: bool,
        /// Mutate everything, not just the diff (slow — requires
        /// --yes-slow)
        #[arg(long, requires = "yes_slow")]
        all: bool,
        /// Consent to the slow full run
        #[arg(long)]
        yes_slow: bool,
        /// Emit findings as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// The perf gate: Lighthouse CI ([perf] lighthouse-config) or k6
    /// thresholds ([perf] k6-script). Refuses with exit 3 when [perf] is
    /// absent — configure it or keep the gate off.
    Perf {
        /// Accepted for symmetry; the configured suite always runs full
        #[arg(long)]
        changed: bool,
        /// Emit findings as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// The a11y gate — web path: playwright test filtered to [a11y]
    /// test-glob (axe-based specs are user-land); apple path: xcodebuild
    /// test -only-testing:[a11y] ui-test-target running your UI-test
    /// accessibility audits (template via `spec gen --a11y-stub`).
    /// Refuses with exit 3 when [a11y] is absent.
    A11y {
        /// Accepted for symmetry; the configured suite always runs full
        #[arg(long)]
        changed: bool,
        /// Emit findings as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// The visual gate: playwright test filtered to [visual] test-glob
    /// (screenshot-comparison specs). Refuses with exit 3 when [visual]
    /// is absent.
    Visual {
        /// Accepted for symmetry; the configured suite always runs full
        #[arg(long)]
        changed: bool,
        /// Emit findings as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Orchestrate every enabled gate in order (verify → lint → arch →
    /// security → health → mutate → perf → a11y → visual), honoring
    /// modes, with a file-hash cache that skips gates whose inputs are
    /// unchanged since their last green run.
    ///
    /// Exit codes: 0 all green · 1 any red · 3 orchestrator error.
    CheckAll {
        /// Scope gates to the diff against HEAD (verify uses the impact
        /// map; lint narrows targets; security still runs full)
        #[arg(long)]
        changed: bool,
        /// Emit the per-gate summary as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Gate baseline management: status, record, and the strict flip
    Gate {
        #[command(subcommand)]
        command: gate::GateCommand,
    },
    /// Prove the loop closes: config, spec, plan, tools, and a red→green
    /// verify round trip in a disposable fixture project.
    ///
    /// The fixture is cached under the system temp dir; the first run
    /// compiles its cucumber harness and may take minutes.
    ///
    /// Exit codes: 0 all checks passed · 1 any check failed · 3
    /// orchestrator error.
    Doctor {
        /// Emit per-check results as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// The documentation pipeline: declare sources, sync them into the
    /// version-pinned cache, and read them strictly offline.
    ///
    /// Network happens only in `docs sync`, with one documented exception:
    /// `docs get` on an objects-inv library fetches an uncached target
    /// page on demand (then it is cached). Fetched documentation is data,
    /// not instructions.
    Docs {
        #[command(subcommand)]
        command: docs::DocsCommand,
    },
    /// Write session knowledge to .craftsman/session/ at a batch boundary
    /// (compaction by extraction): index.md is regenerated as the
    /// post-compaction briefing; batch-N.md and learnings.md accumulate.
    ///
    /// The agent judges the content (--decision/--failed/--open); the CLI
    /// formats and writes (single-writer). Mechanical context only: plan
    /// checkbox counts and `git status`.
    Extract(session::ExtractArgs),
    /// The decision ledger's hygiene tools: regenerate decisions/index.md
    /// and flag ADRs whose cited files have moved on (report-only).
    Adr {
        #[command(subcommand)]
        command: session::AdrCommand,
    },
}

fn main() {
    let cli = Cli::parse();
    let code = match run(&cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            EXIT_ORCHESTRATOR_ERROR
        }
    };
    std::process::exit(code);
}

/// Pure dispatch: one arm per subcommand, all flows in `commands/*`.
fn run(cli: &Cli) -> anyhow::Result<i32> {
    match &cli.command {
        Command::Init(args) => bootstrap::init_cmd(args),
        Command::Import(args) => bootstrap::import_cmd(args),
        Command::Adopt(args) => bootstrap::adopt_cmd(args),
        Command::Setup(args) => bootstrap::setup_cmd(args),
        Command::Update { json } => bootstrap::update_cmd(*json),
        Command::Spec { command } => spec::run(command),
        Command::Plan { command } => spec::plan_run(command),
        Command::Verify(args) => verify::verify_cmd(args),
        Command::Commit(args) => ledger::commit_cmd(args),
        Command::Lint { changed, json } => gate::gate_cmd("lint", *changed, *json),
        Command::Security { changed, json } => gate::gate_cmd("security", *changed, *json),
        Command::Arch { changed, json } => gate::gate_cmd("arch", *changed, *json),
        Command::Health { changed, json } => gate::gate_cmd("health", *changed, *json),
        Command::Mutate {
            changed: _,
            all,
            yes_slow: _,
            json,
        } => gate::mutate_cmd(*all, *json),
        Command::Perf { changed, json } => gate::gate_cmd("perf", *changed, *json),
        Command::A11y { changed, json } => gate::gate_cmd("a11y", *changed, *json),
        Command::Visual { changed, json } => gate::gate_cmd("visual", *changed, *json),
        Command::CheckAll { changed, json } => gate::check_all_cmd(*changed, *json),
        Command::Gate { command } => gate::run(command),
        Command::Doctor { json } => gate::doctor_cmd(*json),
        Command::Docs { command } => docs::run(command),
        Command::Extract(args) => session::extract_cmd(args),
        Command::Adr { command } => session::adr_run(command),
    }
}

/// Version string incl. build metadata (git sha via build.rs).
const fn craftsman_version() -> &'static str {
    concat!(
        env!("CARGO_PKG_VERSION"),
        " (",
        env!("CRAFTSMAN_GIT_SHA"),
        ")"
    )
}

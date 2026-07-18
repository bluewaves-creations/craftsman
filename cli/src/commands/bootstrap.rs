//! `craftsman init | adopt | setup | update` — the Batch 8 bootstrap
//! family.

use clap::Args;

use super::{EXIT_PASS, cwd};

#[derive(Args)]
pub struct InitArgs {
    /// Project name for [project] name
    #[arg(long)]
    pub name: String,
    /// Stack (repeatable): swift-apple | swift | python |
    /// typescript | rust | bash
    #[arg(long = "stack", required = true)]
    pub stack: Vec<String>,
    /// Spec file name
    #[arg(long, default_value = "SPEC.md")]
    pub spec: String,
    /// Overwrite existing scaffold files (still listed in the report)
    #[arg(long)]
    pub force: bool,
    /// Emit the scaffold report as JSON on stdout
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct AdoptArgs {
    /// Report phase state (the default when no flag is given)
    #[arg(long)]
    pub status: bool,
    /// Start phase N (0..=4) — refuses while phase N-1 is incomplete
    #[arg(long, value_name = "N", conflicts_with_all = ["status", "complete_phase"])]
    pub start_phase: Option<u8>,
    /// Record phase N complete
    #[arg(long, value_name = "N", conflicts_with = "status")]
    pub complete_phase: Option<u8>,
    /// Emit the phase report as JSON on stdout
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "clap argument struct: every bool is an independent CLI switch"
)]
pub struct SetupArgs {
    /// Remove installed skills — mirror of install, same proofs
    #[arg(long, conflicts_with = "status")]
    pub remove: bool,
    /// Report what is installed where (no writes)
    #[arg(long)]
    pub status: bool,
    /// Replace/remove entries not attributable to setup (still listed)
    #[arg(long, conflicts_with = "status")]
    pub force: bool,
    /// Emit the report as JSON on stdout
    #[arg(long)]
    pub json: bool,
}

pub fn init_cmd(args: &InitArgs) -> anyhow::Result<i32> {
    let request = craftsman::bootstrap::init::Request {
        name: args.name.clone(),
        stacks: args.stack.clone(),
        spec: args.spec.clone(),
        force: args.force,
    };
    let report = craftsman::bootstrap::init::run(&cwd()?, &request)?;
    for f in &report.files {
        eprintln!("{:>12}  {}", f.action, f.path);
    }
    eprintln!("init: scaffolded {} in {}", request.name, report.root);
    for step in &report.next {
        eprintln!("next: {step}");
    }
    if args.json {
        println!("{:#}", serde_json::json!(report));
    }
    Ok(EXIT_PASS)
}

pub fn adopt_cmd(args: &AdoptArgs) -> anyhow::Result<i32> {
    use craftsman::bootstrap::adopt;

    let cwd = cwd()?;
    let report = match (args.start_phase, args.complete_phase) {
        (Some(n), None) => adopt::start_phase(&cwd, n)?,
        (None, Some(n)) => adopt::complete_phase(&cwd, n)?,
        _ => adopt::status(&cwd)?,
    };
    for action in &report.actions {
        eprintln!("  {action}");
    }
    for (n, label) in adopt::PHASES {
        let record = report.phases.iter().find(|p| p.phase == n);
        let (mark, detail) = record.map_or(("todo", String::new()), |r| {
            r.completed_at.as_ref().map_or_else(
                || {
                    (
                        "now ",
                        format!("  started {} at {}", r.started_at, r.started_head),
                    )
                },
                |done| ("done", format!("  completed {done}")),
            )
        });
        eprintln!("  {mark}  {n} {label:<14}{detail}");
    }
    match report.next_phase {
        Some(n) => eprintln!("adopt: next phase is {n} — see the craftsman-init adopt gear"),
        None => eprintln!("adopt: all five phases complete — steady state"),
    }
    if args.json {
        println!("{:#}", serde_json::json!(report));
    }
    Ok(EXIT_PASS)
}

pub fn setup_cmd(args: &SetupArgs) -> anyhow::Result<i32> {
    use craftsman::bootstrap::setup;

    let home = setup::home()?;
    let report = if args.status {
        setup::status(&home)?
    } else if args.remove {
        setup::remove(&home, args.force)?
    } else {
        setup::install(&home, args.force)?
    };
    print_setup_report(&report, args.json);
    Ok(EXIT_PASS)
}

fn print_setup_report(report: &craftsman::bootstrap::setup::Report, json: bool) {
    for r in &report.rows {
        eprintln!(
            "  {:<12} {:<12} {:<22} {}",
            r.scope, r.action, r.skill, r.detail
        );
    }
    eprintln!(
        "setup: craftsman {} — canonical skills at {}",
        report.version, report.canonical_dir
    );
    if json {
        println!("{:#}", serde_json::json!(report));
    }
}

pub fn update_cmd(json: bool) -> anyhow::Result<i32> {
    use craftsman::bootstrap::setup;

    eprintln!("craftsman {}", crate::craftsman_version());
    eprintln!(
        "update: no self-update channel yet (team-local phase) — to update the \
         binary, download the current GitHub Release via install.sh or run \
         `cargo install --path cli` from the repo, then re-run `craftsman update`"
    );
    eprintln!("update: refreshing installed skills from this binary's embedded payload…");
    let home = setup::home()?;
    let report = setup::install(&home, false)?;
    print_setup_report(&report, json);
    Ok(EXIT_PASS)
}

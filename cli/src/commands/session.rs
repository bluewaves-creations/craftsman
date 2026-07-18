//! `craftsman extract` and `craftsman adr` — session knowledge and the
//! decision ledger's hygiene tools.

use clap::{Args, Subcommand};

use super::{EXIT_PASS, load};

#[derive(Args)]
pub struct ExtractArgs {
    /// The batch this extract closes (writes/extends batch-N.md)
    #[arg(long)]
    pub batch: Option<u32>,
    /// A decision made this session (repeatable)
    #[arg(long)]
    pub decision: Vec<String>,
    /// A failed approach worth remembering (repeatable; appends to
    /// learnings.md)
    #[arg(long)]
    pub failed: Vec<String>,
    /// An open question for the next session (repeatable)
    #[arg(long)]
    pub open: Vec<String>,
    /// Print the current index.md instead of writing anything
    #[arg(long, conflicts_with_all = ["batch", "decision", "failed", "open"])]
    pub show: bool,
    /// Emit the written-file report as JSON on stdout
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum AdrCommand {
    /// Regenerate decisions/index.md — one line per decision (first
    /// heading + Status), warning past the 500-token budget.
    Index {
        /// Emit the report as JSON on stdout
        #[arg(long)]
        json: bool,
    },
    /// Cross-reference active ADRs against the git history of files they
    /// cite; more than [adr] stale-commits (default 10) later commits →
    /// "confirm or supersede". Report-only: findings exit 0.
    Stale {
        /// Emit findings as JSON on stdout
        #[arg(long)]
        json: bool,
    },
}

pub fn extract_cmd(args: &ExtractArgs) -> anyhow::Result<i32> {
    if args.show {
        let loaded = load()?;
        print!("{}", craftsman::session::show(&loaded.root)?);
        return Ok(EXIT_PASS);
    }
    let request = craftsman::session::ExtractRequest {
        batch: args.batch,
        decisions: args.decision.clone(),
        failed: args.failed.clone(),
        open: args.open.clone(),
    };
    let loaded = load()?;
    let report = craftsman::session::extract(&loaded.root, &loaded.config, &request)?;
    eprintln!("extract: wrote {}", report.index);
    if let Some(batch) = &report.batch_file {
        eprintln!("extract: extended {batch}");
    }
    if report.learnings_appended > 0 {
        eprintln!(
            "extract: appended {} learning(s) to .craftsman/session/learnings.md",
            report.learnings_appended
        );
    }
    if args.json {
        println!("{:#}", serde_json::json!(report));
    }
    Ok(EXIT_PASS)
}

/// Dispatch `craftsman adr <index|stale>`.
pub fn adr_run(command: &AdrCommand) -> anyhow::Result<i32> {
    match command {
        AdrCommand::Index { json } => adr_index_cmd(*json),
        AdrCommand::Stale { json } => adr_stale_cmd(*json),
    }
}

fn adr_index_cmd(json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let report = craftsman::adr::index(&loaded.root, &loaded.config)?;
    for d in &report.decisions {
        eprintln!("  {:<10} {}", d.status, d.title);
    }
    eprintln!(
        "adr index: wrote {} — {} decision(s), ~{} tokens",
        report.path,
        report.decisions.len(),
        report.token_estimate
    );
    if report.over_budget {
        eprintln!(
            "warning: the index estimates over the 500-token budget — \
             consolidate decisions (record → consolidate → supersede)"
        );
    }
    if json {
        println!("{:#}", serde_json::json!(report));
    }
    Ok(EXIT_PASS)
}

fn adr_stale_cmd(json: bool) -> anyhow::Result<i32> {
    let loaded = load()?;
    let findings = craftsman::adr::stale(&loaded.root, &loaded.config)?;
    for f in &findings {
        eprintln!("  stale?  {} — {}", f.file, f.advice);
        eprintln!("          cited: {}", f.cited.join(", "));
    }
    eprintln!(
        "adr stale: {} advisory finding(s) (threshold {} commits; report-only)",
        findings.len(),
        loaded.config.adr.stale_commits()
    );
    if json {
        println!("{:#}", serde_json::json!({ "findings": findings }));
    }
    Ok(EXIT_PASS)
}

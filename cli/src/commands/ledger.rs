//! `craftsman commit` — the structured ledger commit.

use anyhow::Context as _;
use clap::Args;

use craftsman::ledger::{self, CommitRequest, CommitType};

use super::{EXIT_PASS, EXIT_VERIFICATION_FAILURE, cwd};

#[derive(Args)]
pub struct CommitArgs {
    /// Conventional commit type
    #[arg(long = "type", value_enum)]
    pub commit_type: CommitType,
    /// Optional scope, e.g. `batch-3` → `feat(batch-3): …`
    #[arg(long)]
    pub scope: Option<String>,
    /// Commit subject line
    #[arg(long)]
    pub message: String,
    /// Body line (repeatable, in order)
    #[arg(long)]
    pub body: Vec<String>,
    /// Read the body from a file instead of --body lines
    #[arg(long, conflicts_with = "body")]
    pub body_file: Option<std::path::PathBuf>,
    /// `Scenarios:` trailer value (repeatable)
    #[arg(long)]
    pub scenarios: Vec<String>,
    /// `Learned:` trailer value (repeatable)
    #[arg(long)]
    pub learned: Vec<String>,
    /// `Rejected:` trailer value (repeatable)
    #[arg(long)]
    pub rejected: Vec<String>,
    /// `Ref:` trailer value (repeatable)
    #[arg(long = "ref")]
    pub refs: Vec<String>,
    /// `Dependency:` trailer value (repeatable)
    #[arg(long)]
    pub dependency: Vec<String>,
    /// Emit {committed, sha, gates} as JSON on stdout
    #[arg(long)]
    pub json: bool,
}

pub fn commit_cmd(args: &CommitArgs) -> anyhow::Result<i32> {
    let body = match &args.body_file {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("cannot read --body-file {}", path.display()))?
            .lines()
            .map(str::to_owned)
            .collect(),
        None => args.body.clone(),
    };
    let request = CommitRequest {
        commit_type: args.commit_type,
        scope: args.scope.clone(),
        subject: args.message.clone(),
        body,
        scenarios: args.scenarios.clone(),
        learned: args.learned.clone(),
        rejected: args.rejected.clone(),
        refs: args.refs.clone(),
        dependencies: args.dependency.clone(),
    };
    let report = ledger::commit(&cwd()?, &request)?;
    Ok(print_report(&report, args.json))
}

/// Report the attempt on stderr (and stdout as JSON when asked) and
/// return the exit code.
fn print_report(report: &ledger::CommitReport, json: bool) -> i32 {
    for g in &report.gates {
        if g.passed {
            eprintln!("gate {}: ok ({})", g.gate, g.detail);
        } else {
            eprintln!("gate {} FAILED:\n{}", g.gate, g.detail);
        }
    }
    if json {
        let doc = serde_json::json!({
            "committed": report.committed,
            "sha": report.sha,
            "subject": report.subject,
            "gates": report.gates,
        });
        println!("{doc:#}");
    }
    let code = if report.committed {
        let sha = report.sha.as_deref().unwrap_or("");
        let short = &sha[..sha.len().min(9)];
        eprintln!("committed {short} {}", report.subject);
        EXIT_PASS
    } else {
        let red = report
            .gates
            .iter()
            .filter(|g| !g.passed)
            .map(|g| g.gate.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("commit refused — red gate: {red} (nothing committed)");
        EXIT_VERIFICATION_FAILURE
    };
    // Boundary observability: pure visibility, printed whether or not the
    // commit landed — never a threshold, never a block.
    if let Some(line) = craftsman::session::distance_line(&report.root) {
        eprintln!("{line}");
    }
    code
}

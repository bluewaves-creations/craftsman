use clap::Parser;

/// The Craftsman Dev CLI — mechanical verification for agentic development.
///
/// Exit codes: 0 pass · 1 verification failure · 2 usage error ·
/// 3 orchestrator error · 4 empty selection.
#[derive(Parser)]
#[command(name = "craftsman", version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}

mod mcp;
mod repl;
mod session;
mod session_id;
mod wolfram;

use clap::{Parser, Subcommand};
use tracing::{Level, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "mathematica-mcp-server")]
#[command(about = "MCP server exposing Wolfram/Mathematica via WSTP", long_about = None)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the MCP server over stdio (for Continue/Claude Desktop/etc.)
    Serve,
    /// Interactive REPL that calls the same tools locally (no MCP host needed)
    Repl,
}

fn init_tracing() {
    // IMPORTANT: server mode must not write to stdout (stdio transport uses stdout for protocol).
    // tracing-subscriber defaults to stderr; we keep it that way.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_level(true)
        .with_writer(std::io::stderr)
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let cmd = cli.cmd.unwrap_or(Command::Serve);

    match cmd {
        Command::Serve => {
            info!("starting MCP server (stdio)");
            mcp::run_server().await?;
        }
        Command::Repl => {
            info!("starting local REPL");
            repl::run_repl().await?;
        }
    }

    Ok(())
}

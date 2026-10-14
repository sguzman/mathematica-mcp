use clap::{Parser, Subcommand};
use std::io::IsTerminal;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "mathematica-mcp-server")]
#[command(about = "MCP server exposing Wolfram/Mathematica via WSTP", long_about = None)]
struct Cli {
  #[command(subcommand)]
  cmd: Option<Command>
}

#[derive(Subcommand, Debug)]
enum Command {
  /// Run the MCP server over stdio.
  Serve,
  /// Interactive REPL, sharing the library's session engine.
  Repl
}

fn init_tracing() {
  // Never log to stdout: MCP owns stdout for JSON-RPC frames.
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
  let cmd = match cli.cmd {
    | Some(cmd) => cmd,
    | None if std::io::stdin().is_terminal() => Command::Repl,
    | None => Command::Serve
  };
  match cmd {
    | Command::Serve => {
      info!("starting MCP server (stdio)");
      mathematica_mcp::serve_stdio().await?
    }
    | Command::Repl => {
      info!("starting local REPL");
      mathematica_mcp::run_repl().await?
    }
  }
  Ok(())
}

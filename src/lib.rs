//! Embed Wolfram Mathematica in Rust applications or expose it through MCP.
//!
//! This crate provides an async, native WSTP-backed `Mathematica` API. The
//! existing MCP server and interactive REPL are thin clients of the same engine.
//!
//! ```no_run
//! use mathematica_mcp::Mathematica;
//! use std::time::Duration;
//!
//! # async fn demo() -> anyhow::Result<()> {
//! let client = Mathematica::new()?; // Must be inside a Tokio runtime.
//! let session = client.create_session().await?;
//! let result = client.execute(&session, "1 + 1", Duration::from_secs(20)).await?;
//! assert_eq!(result.output, "2");
//! client.close_session(&session).await?;
//! # Ok(())
//! # }
//! ```
//!
//! **Security:** Wolfram evaluation is arbitrary code execution with the
//! permissions of the calling user. Do not expose it to untrusted inputs.
//!
//! **Native dependency:** Linux consumers need a local licensed Wolfram kernel
//! and WSTP SDK. The compiler must resolve/link WSTP on the consuming machine.

mod api;
mod config;
mod mcp;
mod platform;
mod repl;
mod session;
mod session_id;
mod wolfram;

pub use api::{FinanceQuery, Mathematica};
pub use session::SessionInfo;
pub use wolfram::{EvalResult, build_financial_data_code};

/// Run the MCP stdio adapter using the public Mathematica library engine.
///
/// This writes JSON-RPC to stdout; logs are routed to stderr by the CLI.
pub async fn serve_stdio() -> anyhow::Result<()> {
  mcp::run_server().await
}

/// Run the interactive Mathematica REPL using the same engine.
pub async fn run_repl() -> anyhow::Result<()> {
  repl::run_repl().await
}

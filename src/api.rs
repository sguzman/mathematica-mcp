//! Public async client facade shared by embedded Rust callers and MCP.
use std::time::Duration;

use anyhow::{Context, anyhow};
use crate::session::{SessionInfo, SessionManager};
use crate::wolfram::{self, EvalResult};

/// A typed request for Mathematica's `FinancialData` function.
///
/// Dates, when specified, must be ISO `YYYY-MM-DD` strings and supplied
/// together. This type only builds an expression; the underlying Wolfram
/// service may need network access.
#[derive(Debug, Clone, Copy)]
pub struct FinanceQuery<'a> {
  pub symbol: &'a str,
  pub property: Option<&'a str>,
  pub start_date: Option<&'a str>,
  pub end_date: Option<&'a str>,
  pub interval: Option<&'a str>
}

impl<'a> FinanceQuery<'a> {
  pub fn new(symbol: &'a str) -> Self {
    Self {
      symbol,
      property: None,
      start_date: None,
      end_date: None,
      interval: None
    }
  }
}

/// A native WSTP-backed Mathematica session manager.
///
/// Clone this handle to share its sessions between application components.
/// A Tokio runtime must be active when constructing it; its asynchronous
/// methods should be awaited rather than called from blocking threads.
///
/// Session IDs remain valid only for this manager instance (and its clones).
/// Sessions idle for more than 30 minutes are automatically retired.
///
/// Evaluations are serial per session. A concurrent request to the same
/// session is rejected rather than queued. Kernel-side `TimeConstrained`
/// and an outer watchdog bound normal execution, but stuck native WSTP
/// operations may not be forcibly terminated.
///
/// Wolfram expressions run with the application's OS permissions: never
/// pass untrusted arbitrary input.
#[derive(Clone)]
pub struct Mathematica {
  sessions: SessionManager
}

impl Mathematica {
  /// Create an independent session manager using the current Tokio runtime.
  ///
  /// Construct from `#[tokio::main]`, `tokio::spawn` or an equivalent
  /// runtime context. This does not start a Wolfram kernel until
  /// `create_session` is called.
  pub fn new() -> anyhow::Result<Self> {
    tokio::runtime::Handle::try_current()
      .context("Mathematica::new requires an active Tokio runtime")?;
    Ok(Self {
      sessions: SessionManager::new()
    })
  }

  /// Start a Wolfram kernel, returning its unforgeable per-manager session ID.
  pub async fn create_session(&self) -> anyhow::Result<String> {
    self.sessions.create_session().await
  }

  /// Evaluate Wolfram Language code and return text, printed logs, and an
  /// optional Base64 PNG for supported graphics expressions.
  ///
  /// `timeout` must be between 1 and 3600 seconds. Each session can process
  /// only one evaluation at a time. A session which never responds is retired.
  pub async fn execute(
    &self,
    session_id: &str,
    code: &str,
    timeout: Duration
  ) -> anyhow::Result<EvalResult> {
    if !self.sessions.verify(session_id) {
      return Err(anyhow!("invalid session ID (malformed, tampered, or from another manager)"));
    }
    self.sessions.eval(session_id, code, timeout).await
  }

  /// Build and evaluate a Wolfram `FinancialData` expression.
  ///
  /// The builder validates ISO dates and escapes quoted symbol/property input.
  /// The Wolfram `FinancialData` service itself may require connectivity.
  pub async fn get_finance(
    &self,
    session_id: &str,
    query: FinanceQuery<'_>,
    timeout: Duration
  ) -> anyhow::Result<EvalResult> {
    let code = wolfram::build_financial_data_code(
      query.symbol, query.property, query.start_date, query.end_date, query.interval
    )?;
    self.execute(session_id, &code, timeout).await
  }

  /// List currently tracked sessions, including their creation times.
  pub async fn list_sessions(&self) -> Vec<SessionInfo> {
    self.sessions.list_sessions().await
  }

  /// Shut down a session and wait for its worker to exit, within bounded
  /// shutdown deadlines. Returns an error if the worker remains stuck.
  pub async fn close_session(&self, session_id: &str) -> anyhow::Result<()> {
    if !self.sessions.verify(session_id) {
      return Err(anyhow!("invalid session ID (malformed, tampered, or from another manager)"));
    }
    self.sessions.close_session(session_id).await
  }
}

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use axum::{Router, routing::get};
use rmcp::transport::streamable_http_server::{
  StreamableHttpServerConfig, StreamableHttpService,
  session::local::LocalSessionManager
};
use tokio::sync::Mutex;

use chrono::Local;
use rmcp::handler::server::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
  ServerCapabilities,
  ServerInfo
};
use rmcp::transport::stdio;
use rmcp::{
  Json,
  serve_server,
  tool,
  tool_handler,
  tool_router
};
use serde::{
  Deserialize,
  Serialize
};

use crate::Mathematica;
use crate::wolfram;

#[derive(Clone)]
pub struct MathematicaServer {
  sessions:    Mathematica,
  owned_sessions: Arc<Mutex<HashSet<String>>>,
  tool_router: ToolRouter<Self>
}

impl MathematicaServer {
  pub fn new() -> anyhow::Result<Self> {
    Ok(Self::with_engine(Mathematica::new()?))
  }

  // An MCP client owns a dedicated handler, but all HTTP clients share one
  // Mathematica engine. Ownership is enforced at the tool boundary.
  fn with_engine(sessions: Mathematica) -> Self {
    Self {
      sessions,
      owned_sessions: Arc::new(Mutex::new(HashSet::new())),
      tool_router: Self::tool_router()
    }
  }

  async fn require_ownership(&self, session_id: &str) -> Result<(), String> {
    if self.owned_sessions.lock().await.contains(session_id) {
      Ok(())
    } else {
      Err("session not found for this MCP client".to_string())
    }
  }
}

#[tool_router]
impl MathematicaServer {
  #[tool(
    name = "mathematica_create_session",
    description = "Launch a new Wolfram kernel session. Returns a session id that must be used \
                   for subsequent calls. Sessions idle for more than 30 minutes are automatically \
                   closed."
  )]
  async fn create_session(&self) -> Result<Json<CreateSessionResult>, String> {
    let id = self.sessions.create_session().await.map_err(|e| e.to_string())?;
    self.owned_sessions.lock().await.insert(id.clone());
    Ok(Json(CreateSessionResult {
      session_id: id
    }))
  }

  #[tool(
    name = "mathematica_execute_code",
    description = "Evaluate Wolfram Language code in a specific session. Returns the result as a \
                   string, captures any printed logs (from Print[] or Message[]), and detects \
                   graphics objects (returning them as Base64 PNGs)."
  )]
  async fn execute_code(
    &self,
    Parameters(params): Parameters<ExecuteParams>
  ) -> Result<Json<ExecuteResult>, String> {
    self.require_ownership(&params.session_id).await?;
    let timeout = std::time::Duration::from_secs(params.timeout_seconds.unwrap_or(60));
    let started = std::time::Instant::now();
    let res = self
      .sessions
      .execute(&params.session_id, &params.code, timeout)
      .await
      .map_err(|e| e.to_string())?;

    Ok(Json(ExecuteResult {
      output:     res.output,
      logs:       res.logs,
      graphics:   res.graphics,
      elapsed_ms: started.elapsed().as_millis() as u64
    }))
  }

  #[tool(
    name = "mathematica_close_session",
    description = "Shutdown a specific Wolfram kernel session and release its resources."
  )]
  async fn close_session(
    &self,
    Parameters(params): Parameters<CloseSessionParams>
  ) -> Result<Json<CloseSessionResult>, String> {
    self.require_ownership(&params.session_id).await?;
    self.sessions.close_session(&params.session_id).await.map_err(|e| e.to_string())?;
    self.owned_sessions.lock().await.remove(&params.session_id);
    Ok(Json(CloseSessionResult {
      closed:     true,
      session_id: params.session_id
    }))
  }

  #[tool(
    name = "mathematica_list_sessions",
    description = "Return all active sessions, their creation times, and how long they have been \
                   idle."
  )]
  async fn list_sessions(&self) -> Result<Json<ListSessionsResult>, String> {
    let owned = self.owned_sessions.lock().await.clone();
    let sessions = self.sessions.list_sessions().await.into_iter()
      .filter(|session| owned.contains(&session.session_id))
      .collect();
    Ok(Json(ListSessionsResult {
      sessions
    }))
  }

  #[tool(
    name = "mathematica_time",
    description = "Return current local and UTC time in RFC3339 format."
  )]
  async fn time(&self) -> Result<Json<TimeResult>, String> {
    let now_local = Local::now();
    Ok(Json(TimeResult {
      local_rfc3339: now_local.to_rfc3339(),
      utc_rfc3339:   chrono::Utc::now().to_rfc3339()
    }))
  }

  #[tool(
    name = "mathematica_get_finance",
    description = "High-level helper for FinancialData[...]. Automatically builds and evaluates \
                   the appropriate Wolfram Language code in the specified session."
  )]
  async fn get_finance(
    &self,
    Parameters(params): Parameters<FinanceParams>
  ) -> Result<Json<FinanceResult>, String> {
    let code = wolfram::build_financial_data_code(
      &params.symbol,
      params.property.as_deref(),
      params.start_date.as_deref(),
      params.end_date.as_deref(),
      params.interval.as_deref()
    )
    .map_err(|e| e.to_string())?;

    self.require_ownership(&params.session_id).await?;
    let timeout = std::time::Duration::from_secs(params.timeout_seconds.unwrap_or(60));
    let started = std::time::Instant::now();
    let res =
      self.sessions.execute(&params.session_id, &code, timeout).await.map_err(|e| e.to_string())?;

    Ok(Json(FinanceResult {
      wolfram_code: code,
      output:       res.output,
      logs:         res.logs,
      graphics:     res.graphics,
      elapsed_ms:   started.elapsed().as_millis() as u64
    }))
  }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for MathematicaServer {
  fn get_info(&self) -> ServerInfo {
    ServerInfo {
      instructions: Some(
        "Mathematica/Wolfram MCP server. Use mathematica_create_session, then \
         mathematica_execute_code / mathematica_get_finance, then mathematica_close_session."
          .to_string()
      ),
      capabilities: ServerCapabilities::builder().enable_tools().build(),
      ..Default::default()
    }
  }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExecuteParams {
  pub session_id:      String,
  pub code:            String,
  pub timeout_seconds: Option<u64>
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CloseSessionParams {
  pub session_id: String
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FinanceParams {
  pub session_id:      String,
  pub symbol:          String,
  pub property:        Option<String>, // e.g. "Close"
  pub start_date:      Option<String>, // "YYYY-MM-DD"
  pub end_date:        Option<String>, // "YYYY-MM-DD"
  pub interval:        Option<String>, // optional
  pub timeout_seconds: Option<u64>
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CreateSessionResult {
  pub session_id: String
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ExecuteResult {
  pub output:     String,
  pub logs:       Vec<String>,
  pub graphics:   Option<String>,
  pub elapsed_ms: u64
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CloseSessionResult {
  pub closed:     bool,
  pub session_id: String
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListSessionsResult {
  pub sessions: Vec<crate::session::SessionInfo>
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TimeResult {
  pub local_rfc3339: String,
  pub utc_rfc3339:   String
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FinanceResult {
  pub wolfram_code: String,
  pub output:       String,
  pub logs:         Vec<String>,
  pub graphics:     Option<String>,
  pub elapsed_ms:   u64
}

pub async fn run_server() -> anyhow::Result<()> {
  let server = MathematicaServer::new()?;

  // Serve over stdio.
  let transport = stdio();
  let running = serve_server(server, transport).await?;
  running.waiting().await?;

  Ok(())
}

 
/// One persistent HTTP listener, one shared kernel manager, and one isolated
/// MCP handler per client. Kernel sessions are created lazily by each client.
pub async fn run_http() -> anyhow::Result<()> {
  let settings = crate::config::load_config()?
    .map(|loaded| loaded.config)
    .unwrap_or_default();
  let address: SocketAddr = settings.server.listen.parse()
    .with_context(|| format!("invalid [server].listen: {}", settings.server.listen))?;
  let endpoint = settings.server.endpoint.trim();
  anyhow::ensure!(
    endpoint.starts_with('/') && endpoint != "/" && !endpoint.ends_with('/'),
    "[server].endpoint must start with / and must not be / or end with /"
  );
  anyhow::ensure!(settings.tenancy.max_kernels > 0 && settings.tenancy.max_kernels <= 128,
    "[tenancy].max_kernels must be between 1 and 128");

  let shared_engine = Mathematica::with_kernel_limit(Some(settings.tenancy.max_kernels))?;
  let http = StreamableHttpService::new(
    move || Ok(MathematicaServer::with_engine(shared_engine.clone())),
    LocalSessionManager::default().into(),
    StreamableHttpServerConfig::default()
  );
  let router = Router::new()
    .route("/healthz", get(|| async { "ok" }))
    .nest_service(endpoint, http);
  let listener = tokio::net::TcpListener::bind(address).await
    .with_context(|| format!("could not bind Mathematica MCP to {address}"))?;
  tracing::info!(%address, endpoint, "Mathematica MCP listening (Streamable HTTP)");
  axum::serve(listener, router)
    .with_graceful_shutdown(shutdown_signal())
    .await?;
  Ok(())
}

async fn shutdown_signal() {
  #[cfg(unix)]
  {
    use tokio::signal::unix::{SignalKind, signal};
    match signal(SignalKind::terminate()) {
      Ok(mut terminate) => {
        tokio::select! {
          _ = tokio::signal::ctrl_c() => {},
          _ = terminate.recv() => {}
        }
      }
      Err(error) => {
        tracing::warn!(%error, "could not register SIGTERM handler");
        let _ = tokio::signal::ctrl_c().await;
      }
    }
  }
  #[cfg(not(unix))]
  {
    let _ = tokio::signal::ctrl_c().await;
  }
}

#[cfg(test)]
mod tenancy_tests {
  use super::*;

  #[tokio::test]
  async fn tenant_scope_rejects_foreign_sessions() {
    let engine = Mathematica::new().unwrap();
    let first = MathematicaServer::with_engine(engine.clone());
    let second = MathematicaServer::with_engine(engine);
    let id = "example-owned-id".to_string();
    first.owned_sessions.lock().await.insert(id.clone());
    assert!(first.require_ownership(&id).await.is_ok());
    assert!(second.require_ownership(&id).await.is_err());
  }
}

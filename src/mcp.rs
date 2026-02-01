use crate::session::SessionManager;
use crate::wolfram;

use chrono::Local;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::service::{RequestContext, Role, Service};
use rmcp::transport::stdio::stdio;
use rmcp::{
    handler::server::ServerHandler, model::Json, schemars, serde, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct MathematicaServer {
    sessions: SessionManager,
}

impl MathematicaServer {
    pub fn new() -> Self {
        Self {
            sessions: SessionManager::new(),
        }
    }
}

#[tool_router]
impl MathematicaServer {
    #[tool(name = "mathematica.create_session")]
    async fn create_session(&self) -> Result<Json<CreateSessionResult>, String> {
        let id = self
            .sessions
            .create_session()
            .await
            .map_err(|e| e.to_string())?;
        Ok(Json(CreateSessionResult { session_id: id }))
    }

    #[tool(name = "mathematica.execute_code")]
    async fn execute_code(&self, params: ExecuteParams) -> Result<Json<ExecuteResult>, String> {
        if !self.sessions.verify(&params.session_id) {
            return Err("Invalid session ID (malformed or tampered).".to_string());
        }

        let started = std::time::Instant::now();
        let out = self
            .sessions
            .eval(&params.session_id, &params.code)
            .await
            .map_err(|e| e.to_string())?;

        Ok(Json(ExecuteResult {
            output: out,
            elapsed_ms: started.elapsed().as_millis() as u64,
        }))
    }

    #[tool(name = "mathematica.close_session")]
    async fn close_session(
        &self,
        params: CloseSessionParams,
    ) -> Result<Json<CloseSessionResult>, String> {
        if !self.sessions.verify(&params.session_id) {
            return Err("Invalid session ID.".to_string());
        }
        self.sessions
            .close_session(&params.session_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Json(CloseSessionResult {
            closed: true,
            session_id: params.session_id,
        }))
    }

    #[tool(name = "mathematica.list_sessions")]
    async fn list_sessions(&self) -> Result<Json<ListSessionsResult>, String> {
        let sessions = self.sessions.list_sessions().await;
        Ok(Json(ListSessionsResult { sessions }))
    }

    #[tool(name = "mathematica.time")]
    async fn time(&self) -> Result<Json<TimeResult>, String> {
        let now_local = Local::now();
        Ok(Json(TimeResult {
            local_rfc3339: now_local.to_rfc3339(),
            utc_rfc3339: chrono::Utc::now().to_rfc3339(),
        }))
    }

    #[tool(name = "mathematica.get_finance")]
    async fn get_finance(&self, params: FinanceParams) -> Result<Json<FinanceResult>, String> {
        if !self.sessions.verify(&params.session_id) {
            return Err("Invalid session ID.".to_string());
        }

        let code = wolfram::build_financial_data_code(
            &params.symbol,
            params.property.as_deref(),
            params.start_date.as_deref(),
            params.end_date.as_deref(),
            params.interval.as_deref(),
        )
        .map_err(|e| e.to_string())?;

        let started = std::time::Instant::now();
        let out = self
            .sessions
            .eval(&params.session_id, &code)
            .await
            .map_err(|e| e.to_string())?;

        Ok(Json(FinanceResult {
            wolfram_code: code,
            output: out,
            elapsed_ms: started.elapsed().as_millis() as u64,
        }))
    }
}

#[tool_handler]
impl ServerHandler for MathematicaServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            // You can add richer text here if you want the client to understand semantics.
            instructions: Some(
                "Mathematica/Wolfram MCP server. Use mathematica.create_session, then mathematica.execute_code / mathematica.get_finance, then mathematica.close_session.".to_string()
            ),
            capabilities: ServerCapabilities::default(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExecuteParams {
    pub session_id: String,
    pub code: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CloseSessionParams {
    pub session_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FinanceParams {
    pub session_id: String,
    pub symbol: String,
    pub property: Option<String>,   // e.g. "Close"
    pub start_date: Option<String>, // "YYYY-MM-DD"
    pub end_date: Option<String>,   // "YYYY-MM-DD"
    pub interval: Option<String>,   // optional
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CreateSessionResult {
    pub session_id: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ExecuteResult {
    pub output: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CloseSessionResult {
    pub closed: bool,
    pub session_id: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListSessionsResult {
    pub sessions: Vec<crate::session::SessionInfo>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TimeResult {
    pub local_rfc3339: String,
    pub utc_rfc3339: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FinanceResult {
    pub wolfram_code: String,
    pub output: String,
    pub elapsed_ms: u64,
}

pub async fn run_server() -> anyhow::Result<()> {
    let server = MathematicaServer::new();
    let service = Service::new(server);

    // Serve over stdio.
    let transport = stdio();
    let running = service.serve(transport).await?;
    running.waiting().await?;

    Ok(())
}

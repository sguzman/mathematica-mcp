//! Independent Rust consumer executable: exercises exported library API.
use std::time::Duration;
use mathematica_mcp::{FinanceQuery, Mathematica};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let engine = Mathematica::new()?;
    let session_id = engine.create_session().await?;
    let value = engine.execute(&session_id, "40 + 2", Duration::from_secs(20)).await?;
    anyhow::ensure!(value.output == "42", "wrong Wolfram result: {}", value.output);
    let sessions = engine.list_sessions().await;
    anyhow::ensure!(sessions.iter().any(|s| s.session_id == session_id), "missing session");
    let finance = FinanceQuery::new("AAPL");
    anyhow::ensure!(finance.symbol == "AAPL");
    engine.close_session(&session_id).await?;
    println!("PASS: independent Rust project consumed Mathematica library and Wolfram kernel");
    Ok(())
}

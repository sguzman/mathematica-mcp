//! Compile with `cargo check --example embedded`.
//! Run with `cargo run --example embedded` on Linux with Mathematica/WSTP.
use std::time::Duration;
use mathematica_mcp::{FinanceQuery, Mathematica};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let engine = Mathematica::new()?;
  let session_id = engine.create_session().await?;
  let result = engine.execute(&session_id, "1 + 1", Duration::from_secs(20)).await?;
  println!("Wolfram returned: {}", result.output);

  // The library also supports a typed FinancialData query, but this
  // demonstration avoids any dependence on live financial services.
  let _finance_query = FinanceQuery::new("AAPL");

  engine.close_session(&session_id).await?;
  Ok(())
}

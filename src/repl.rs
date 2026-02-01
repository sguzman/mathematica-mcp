use crate::session::SessionManager;
use crate::wolfram;

use std::io::{self, Write};

pub async fn run_repl() -> anyhow::Result<()> {
    let sessions = SessionManager::new();
    let mut active: Option<String> = None;

    eprintln!("mathematica-mcp-server repl");
    eprintln!("Commands:");
    eprintln!("  mathematica.create_session");
    eprintln!("  mathematica.list_sessions");
    eprintln!("  mathematica.time");
    eprintln!("  mathematica.execute_code <Wolfram Language code...>");
    eprintln!(
        "  mathematica.get_finance <SYMBOL> [PROPERTY] [START YYYY-MM-DD] [END YYYY-MM-DD] [INTERVAL]"
    );
    eprintln!("  mathematica.close_session [SESSION_ID]");
    eprintln!("  exit | quit");
    eprintln!();

    loop {
        print!("mathematica> ");
        io::stdout().flush()?; // OK in repl mode (not MCP stdio mode)

        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "exit" || line == "quit" {
            break;
        }

        if line == "mathematica.create_session" {
            let id = sessions.create_session().await?;
            eprintln!("OK session_id={id}");
            active = Some(id);
            continue;
        }

        if line == "mathematica.list_sessions" {
            let list = sessions.list_sessions().await;
            eprintln!("{}", serde_json::to_string_pretty(&list)?);
            continue;
        }

        if line == "mathematica.time" {
            let now = chrono::Local::now().to_rfc3339();
            let utc = chrono::Utc::now().to_rfc3339();
            eprintln!(
                "{}",
                serde_json::json!({"local_rfc3339": now, "utc_rfc3339": utc})
            );
            continue;
        }

        if let Some(rest) = line.strip_prefix("mathematica.execute_code ") {
            let Some(id) = active.as_deref() else {
                eprintln!("ERR no active session. Run mathematica.create_session first.");
                continue;
            };
            let out = sessions.eval(id, rest).await?;
            eprintln!("{out}");
            continue;
        }

        if let Some(rest) = line.strip_prefix("mathematica.get_finance ") {
            let Some(id) = active.as_deref() else {
                eprintln!("ERR no active session. Run mathematica.create_session first.");
                continue;
            };
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.is_empty() {
                eprintln!(
                    "ERR usage: mathematica.get_finance <SYMBOL> [PROPERTY] [START] [END] [INTERVAL]"
                );
                continue;
            }
            let symbol = parts[0];
            let property = parts.get(1).copied();
            let start = parts.get(2).copied();
            let end = parts.get(3).copied();
            let interval = parts.get(4).copied();

            let code = wolfram::build_financial_data_code(symbol, property, start, end, interval)?;
            let out = sessions.eval(id, &code).await?;
            eprintln!("WL: {code}");
            eprintln!("{out}");
            continue;
        }

        if line.starts_with("mathematica.close_session") {
            // allow optional explicit id: mathematica.close_session <id>
            let parts: Vec<&str> = line.split_whitespace().collect();
            let id = if parts.len() >= 2 {
                parts[1].to_string()
            } else if let Some(a) = &active {
                a.clone()
            } else {
                eprintln!("ERR no active session and no id provided.");
                continue;
            };

            sessions.close_session(&id).await?;
            eprintln!("OK closed {id}");
            if active.as_deref() == Some(&id) {
                active = None;
            }
            continue;
        }

        eprintln!("ERR unknown command. Type 'mathematica.time' or 'mathematica.create_session'.");
    }

    Ok(())
}

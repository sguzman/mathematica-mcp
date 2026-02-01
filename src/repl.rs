use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

use crate::session::SessionManager;
use crate::wolfram;

pub async fn run_repl()
-> anyhow::Result<()> {
  let sessions = SessionManager::new();
  let mut active: Option<String> = None;

  eprintln!(
    "mathematica-mcp-server repl"
  );
  eprintln!("Commands:");
  eprintln!(
    "  mathematica.create_session"
  );
  eprintln!(
    "  mathematica.list_sessions"
  );
  eprintln!("  mathematica.time");
  eprintln!(
    "  mathematica.execute_code \
     <Wolfram Language code...>"
  );
  eprintln!(
    "  mathematica.get_finance \
     <SYMBOL> [PROPERTY] [START \
     YYYY-MM-DD] [END YYYY-MM-DD] \
     [INTERVAL]"
  );
  eprintln!(
    "  mathematica.close_session \
     [SESSION_ID]"
  );
  eprintln!("  exit | quit");
  eprintln!();

  let mut rl = DefaultEditor::new()
    .map_err(|e| {
      anyhow::anyhow!(
        "Failed to create readline \
         editor: {}",
        e
      )
    })?;

  loop {
    println!();
    let readline =
      rl.readline("mathematica> ");
    match readline {
      | Ok(line) => {
        let line = line.trim();
        if line.is_empty() {
          continue;
        }
        if line == "exit"
          || line == "quit"
        {
          break;
        }

        if line
          == "mathematica.\
              create_session"
        {
          let id = sessions
            .create_session()
            .await?;
          eprintln!(
            "OK session_id={id}"
          );
          active = Some(id);
          continue;
        }

        if line
          == "mathematica.list_sessions"
        {
          let list = sessions
            .list_sessions()
            .await;
          eprintln!(
            "{}",
            serde_json::to_string_pretty(
              &list
            )?
          );
          continue;
        }

        if line == "mathematica.time" {
          let now =
            chrono::Local::now()
              .to_rfc3339();
          let utc = chrono::Utc::now()
            .to_rfc3339();
          eprintln!(
            "{}",
            serde_json::json!({"local_rfc3339": now, "utc_rfc3339": utc})
          );
          continue;
        }

        if let Some(rest) = line
          .strip_prefix(
            "mathematica.execute_code "
          )
        {
          let Some(id) =
            active.as_deref()
          else {
            eprintln!(
              "ERR no active session. \
               Run mathematica.\
               create_session first."
            );
            continue;
          };
          let out = sessions
            .eval(id, rest)
            .await?;
          eprintln!("{out}");
          continue;
        }

        if let Some(rest) = line
          .strip_prefix(
            "mathematica.get_finance "
          )
        {
          let Some(id) =
            active.as_deref()
          else {
            eprintln!(
              "ERR no active session. \
               Run mathematica.\
               create_session first."
            );
            continue;
          };
          let parts: Vec<&str> = rest
            .split_whitespace()
            .collect();
          if parts.is_empty() {
            eprintln!(
              "ERR usage: \
               mathematica.\
               get_finance <SYMBOL> \
               [PROPERTY] [START] \
               [END] [INTERVAL]"
            );
            continue;
          }
          let symbol = parts[0];
          let property =
            parts.get(1).copied();
          let start =
            parts.get(2).copied();
          let end =
            parts.get(3).copied();
          let interval =
            parts.get(4).copied();

          let code = match wolfram::build_financial_data_code(symbol, property, start, end, interval) {
        Ok(c) => c,
        Err(e) if e.to_string().contains("invalid date") => {
            // Heuristic: if second argument looks like a date, likely the symbol is missing
            let mut warn = String::new();
            if parts.get(1).is_some_and(|s| s.chars().all(|c| c.is_ascii_digit() || c == '-')) {
                warn.push_str("Stock symbol missing or first argument not a ticker. ");
            }
            // Also check which date caused the error
            if parts.get(3).is_some_and(|s| !s.chars().all(|c| c.is_ascii_digit() || c == '-')) {
                warn.push_str(&format!("End date '{}' is not in YYYY-MM-DD format.", parts[3]));
            }
            if warn.is_empty() {
                warn = "Invalid date format encountered.".to_string();
            }
            eprintln!("WARN: {}", warn);
            continue;
        }
        Err(e) => return Err(e),
    };
          let out = sessions
            .eval(id, &code)
            .await?;
          eprintln!("WL: {code}");
          eprintln!("{out}");
          continue;
        }

        if line.starts_with(
          "mathematica.close_session"
        ) {
          // allow optional explicit id:
          // mathematica.close_session
          // <id>
          let parts: Vec<&str> = line
            .split_whitespace()
            .collect();
          let id = if parts.len() >= 2 {
            parts[1].to_string()
          } else if let Some(a) =
            &active
          {
            a.clone()
          } else {
            eprintln!(
              "ERR no active session \
               and no id provided."
            );
            continue;
          };

          sessions
            .close_session(&id)
            .await?;
          eprintln!("OK closed {id}");
          if active.as_deref()
            == Some(&id)
          {
            active = None;
          }
          continue;
        }

        eprintln!(
          "ERR unknown command. Type \
           'mathematica.time' or \
           'mathematica.\
           create_session'."
        );
      }
      | Err(
        ReadlineError::Interrupted
      ) => {
        eprintln!("CTRL-C: exiting...");
        break;
      }
      | Err(ReadlineError::Eof) => {
        eprintln!("EOF: exiting...");
        break;
      }
      | Err(err) => {
        eprintln!("Error: {:?}", err);
        break;
      }
    }
  }

  Ok(())
}

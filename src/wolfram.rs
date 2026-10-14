use std::env;
use std::path::PathBuf;

use anyhow::{
  Context,
  anyhow
};
use chrono::Datelike;
use wolfram_expr::{
  Expr,
  ExprKind
};
use wstp::Link;
use wstp::kernel::WolframKernelProcess;

use crate::{
  config,
  platform
};

pub fn resolve_kernel_cmd() -> anyhow::Result<String> {
  // 1) honor the platform-specific path in the repo/application TOML config
  if let Some(loaded) = config::load_config()?
    && let Some(raw) = loaded.config.platform_kernel_path()
  {
      let path = platform::shellexpand_path(raw)?;
      platform::validate_executable(&path)?;
      tracing::info!(
        config_path = %loaded.path.display(),
        kernel_path = %path.display(),
        "using kernel from TOML config"
      );
      return Ok(path.to_string_lossy().to_string());
  }

  // 2) honor WOLFRAM_KERNEL_PATH as a legacy override
  if let Ok(raw) = env::var("WOLFRAM_KERNEL_PATH") {
    let raw = raw.trim();
    if !raw.is_empty() {
      let path = platform::shellexpand_path(raw)?;
      platform::validate_executable(&path)?;
      tracing::info!(kernel_path = %path.display(), "using kernel from WOLFRAM_KERNEL_PATH");
      return Ok(path.to_string_lossy().to_string());
    }
  }

  // 3) try platform discovery (registry on Windows, etc.)
  if let Some(p) = platform::discover_kernel_path() {
    tracing::info!(kernel_path = %p.display(), "using kernel found via platform discovery");
    return Ok(p.to_string_lossy().to_string());
  }

  // 4) fall back to PATH lookup
  for candidate in platform::get_default_kernel_names() {
    if let Some(p) = find_in_path(candidate) {
      tracing::info!(kernel_path = %p.display(), "using kernel found on PATH");
      return Ok(p.to_string_lossy().to_string());
    }
  }

  // 5) last resort: try bare WolframKernel and let OS/WSTP resolve (may work on
  //    some setups)
  tracing::warn!(
    "no kernel found via config, WOLFRAM_KERNEL_PATH, discovery, or PATH; trying 'WolframKernel' \
     as a fallback"
  );
  Ok("WolframKernel".to_string())
}

pub fn launch_link(kernel_cmd: &str) -> anyhow::Result<WolframKernelProcess> {
  let path = PathBuf::from(kernel_cmd);
  tracing::debug!(kernel_path = %path.display(), "launching Wolfram kernel");
  let kernel =
    WolframKernelProcess::launch(&path).map_err(|e| anyhow!("WSTP launch failed: {e:?}"))?;
  Ok(kernel)
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema, Clone)]
pub struct EvalResult {
  pub output:   String,
  pub logs:     Vec<String>,
  pub graphics: Option<String> // Base64 PNG
}

// One Wolfram deadline covers evaluation AND potentially expensive PNG export.
// The previous wrapper timed only user code, allowing front-end-backed graphics
// rendering to hang the WSTP link outside TimeConstrained.
fn evaluation_wrapper(code: &str, timeout: std::time::Duration) -> String {
  let seconds = timeout.as_secs();
  format!(
    "ExportString[TimeConstrained[Module[{{res, graphics, rendered, graphicsError = Null}}, \
     res = Check[({code}), $Failed]; \
     graphics = If[MemberQ[{{Graphics, Graphics3D, Graph, GeoGraphics, Legended}}, Head[res]], \
       If[PacletFind[\"ImageMetadataTools\"] === {{}}, \
         (graphicsError = \"ImageMetadataTools is not installed. Run python3 -u scripts/install_image_metadata_tools.py --install once, then retry graphics.\"; Null), \
         (Print[\"mathematica-mcp:graphics:start\"]; \
          rendered = Quiet[Check[ExportByteArray[res, \"PNG\"], $Failed]]; \
          Print[\"mathematica-mcp:graphics:end\"]; \
          If[Head[rendered] === ByteArray, BaseEncode[rendered], Null])], \
       Null]; \
     <|\"output\" -> ToString[res, InputForm], \"graphics\" -> graphics, \
     \"graphics_error\" -> graphicsError, \"timed_out\" -> False|>], \
     {seconds}, <|\"output\" -> \"$Aborted\", \"graphics\" -> Null, \
     \"graphics_error\" -> Null, \"timed_out\" -> True|>], \"JSON\"]"
  )
}

fn parse_evaluation_response(json_str: &str, logs: Vec<String>) -> anyhow::Result<EvalResult> {
  let val: serde_json::Value = serde_json::from_str(json_str)?;
  if val["timed_out"].as_bool() == Some(true) {
    return Err(anyhow!("evaluation exceeded Wolfram TimeConstrained limit"));
  }
  if let Some(error) = val["graphics_error"].as_str() {
    return Err(anyhow!("{error}"));
  }
  let output = val["output"]
    .as_str()
    .ok_or_else(|| anyhow!("Wolfram response missing string 'output'"))?
    .to_string();
  let graphics = match &val["graphics"] {
    serde_json::Value::String(s) => Some(s.clone()),
    serde_json::Value::Null => None,
    _ => return Err(anyhow!("Wolfram response graphics must be Base64 text or null"))
  };
  Ok(EvalResult { output, logs, graphics })
}

pub fn evaluate(
  link: &mut Link,
  code: &str,
  timeout: std::time::Duration
) -> anyhow::Result<EvalResult> {
  let wrapper = evaluation_wrapper(code, timeout);
  link
    .put_eval_packet(&Expr::normal(wolfram_expr::Symbol::new("System`ToExpression"), vec![
      Expr::string(&wrapper),
    ]))
    .map_err(|e| anyhow!("put_eval_packet failed: {e:?}"))?;

  link.flush().map_err(|e| anyhow!("flush failed: {e:?}"))?;

  let mut logs = Vec::new();
  loop {
    let pkt = link.raw_next_packet().map_err(|e| anyhow!("raw_next_packet failed: {e:?}"))?;
    match pkt {
      | wstp::sys::RETURNPKT => {
        let result_expr = link.get_expr().map_err(|e| anyhow!("get_expr failed: {e:?}"))?;
        link.new_packet().map_err(|e| anyhow!("new_packet failed: {e:?}"))?;
        let ExprKind::String(json_str) = result_expr.kind() else {
          return Err(anyhow!("expected JSON string from kernel, got: {result_expr:?}"));
        };
        return parse_evaluation_response(json_str, logs);
      }
      | wstp::sys::TEXTPKT => {
        if let Ok(expr) = link.get_expr()
          && let ExprKind::String(s) = expr.kind()
        {
          if matches!(s.as_str(), "mathematica-mcp:graphics:start" | "mathematica-mcp:graphics:end") {
            tracing::info!(stage = %s, "Wolfram graphics stage");
          } else {
            logs.push(s.clone());
          }
        }
        link.new_packet().map_err(|e| anyhow!("new_packet failed: {e:?}"))?;
      }
      | wstp::sys::MESSAGEPKT => {
        link.new_packet().map_err(|e| anyhow!("new_packet failed: {e:?}"))?;
      }
      | _ => {
        link.new_packet().map_err(|e| anyhow!("new_packet failed: {e:?}"))?;
      }
    }
  }
}

#[cfg(test)]
mod evaluation_tests {
  use super::*;

  #[test]
  fn wrapper_limits_graphics_export_as_well_as_evaluation() {
    let wrapper = evaluation_wrapper("Plot[Sin[x], {x, 0, 1}]", std::time::Duration::from_secs(7));
    assert!(wrapper.contains("ExportString[TimeConstrained[Module["));
    assert!(wrapper.contains("res = Check[(Plot[Sin[x], {x, 0, 1}]), $Failed]"));
    assert!(wrapper.contains("ExportByteArray[res, \"PNG\"]"));
    assert!(wrapper.contains("PacletFind[\"ImageMetadataTools\"] === {}"));
    assert!(wrapper.contains("install_image_metadata_tools.py --install"));
    assert!(wrapper.contains("BaseEncode[rendered]"));
    assert!(wrapper.contains("mathematica-mcp:graphics:start"));
    assert!(wrapper.contains("mathematica-mcp:graphics:end"));
    assert!(wrapper.contains("7, <|\"output\" -> \"$Aborted\""));
    assert!(wrapper.contains("\"timed_out\" -> True"));
  }

  #[test]
  fn response_parses_output_logs_and_image() {
    let r = parse_evaluation_response(
      r#"{"output":"42","graphics":"iVBORw0KGgo=","timed_out":false}"#,
      vec!["hello".into()]
    ).unwrap();
    assert_eq!(r.output, "42");
    assert_eq!(r.logs, vec!["hello"]);
    assert_eq!(r.graphics.as_deref(), Some("iVBORw0KGgo="));
  }

  #[test]
  fn response_rejects_timeouts_and_malformed_data() {
    assert!(parse_evaluation_response(
      r#"{"output":"$Aborted","graphics":null,"timed_out":true}"#, vec![]
    ).is_err());
    assert!(parse_evaluation_response(r#"{"graphics":null}"#, vec![]).is_err());
    assert!(parse_evaluation_response("not JSON", vec![]).is_err());
    let missing_paclet = r#"{"output":"Graphics[]","graphics":null,"graphics_error":"ImageMetadataTools is not installed","timed_out":false}"#;
    assert!(parse_evaluation_response(missing_paclet, vec![]).unwrap_err().to_string().contains("ImageMetadataTools"));
  }
}

pub fn build_financial_data_code(
  symbol: &str,
  property: Option<&str>,
  start_date: Option<&str>,
  end_date: Option<&str>,
  interval: Option<&str>
) -> anyhow::Result<String> {
  // FinancialData forms (Wolfram docs):
  // - FinancialData["symbol"]
  // - FinancialData["symbol","property" ]
  // - FinancialData["symbol","property" ,{start,end}]
  // (and some forms accept a
  // time/interval argument depending on
  // data type)

  if symbol.trim().is_empty() {
    return Err(anyhow!("symbol must not be empty"));
  }
  let sym = wl_string(symbol);
  let prop = property.map(wl_string);

  let date_range = match (start_date, end_date) {
    | (Some(s), Some(e)) => {
      let start = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .with_context(|| format!("invalid start_date '{s}', expected YYYY-MM-DD"))?;
      let end = chrono::NaiveDate::parse_from_str(e, "%Y-%m-%d")
        .with_context(|| format!("invalid end_date '{e}', expected YYYY-MM-DD"))?;
      if start > end {
        return Err(anyhow!("start_date must not be after end_date"));
      }
      let s = iso_date_to_dateobject(s)?;
      let e = iso_date_to_dateobject(e)?;
      Some(format!("{{{s}, {e}}}"))
    }
    | (None, None) => None,
    | _ => return Err(anyhow!("start_date and end_date must be supplied together"))
  };

  let mut args = vec![format!("\"{sym}\"")];
  if let Some(p) = prop {
    args.push(format!("\"{p}\""));
  }
  if let Some(dr) = date_range {
    // If property omitted but date
    // range provided, we still need a
    // property. Pick a reasonable
    // default to keep syntax valid.
    if args.len() == 1 {
      args.push("\"Close\"".to_string());
    }
    args.push(dr);
  }
  if let Some(intv) = interval {
    // Optional interval argument
    // (passed as a WL string; WL will
    // interpret known values)
    args.push(format!("\"{}\"", wl_string(intv)));
  }

  Ok(format!("FinancialData[{}]", args.join(", ")))
}

fn iso_date_to_dateobject(s: &str) -> anyhow::Result<String> {
  // Expect YYYY-MM-DD
  let d = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
    .with_context(|| format!("invalid date '{s}', expected YYYY-MM-DD"))?;
  Ok(format!("DateObject[{{{}, {}, {}}}]", d.year(), d.month(), d.day()))
}

fn wl_string(s: &str) -> String {
  // Escape for inclusion inside "..."
  s.replace('\\', "\\\\").replace('\"', "\\\"")
}

fn find_in_path(exe: &str) -> Option<PathBuf> {
  let path = env::var_os("PATH")?;
  for dir in env::split_paths(&path) {
    let cand = dir.join(exe);
    if cand.exists() {
      return Some(cand);
    }
  }
  None
}

#[cfg(test)]
mod finance_tests {
  use super::*;

  #[test]
  fn builds_default_finance_query() {
    assert_eq!(build_financial_data_code("AAPL", None, None, None, None).unwrap(),
      "FinancialData[\"AAPL\"]");
  }

  #[test]
  fn validates_date_ranges() {
    assert!(build_financial_data_code("AAPL", None, Some("2026-01-01"), None, None).is_err());
    assert!(build_financial_data_code("AAPL", None, Some("2026-02-01"), Some("2026-01-01"), None).is_err());
    assert!(build_financial_data_code(" ", None, None, None, None).is_err());
    let query = build_financial_data_code("AAPL", None, Some("2026-01-01"), Some("2026-02-01"), None).unwrap();
    assert!(query.contains("\"Close\""));
    assert!(query.contains("DateObject[{2026, 1, 1}]"));
  }

  #[test]
  fn escapes_symbol_as_wolfram_string() {
    let query = build_financial_data_code("A\"BC", None, None, None, None).unwrap();
    assert!(query.contains("A\\\"BC"));
  }
}

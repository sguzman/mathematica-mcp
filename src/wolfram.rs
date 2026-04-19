use std::path::PathBuf;
use std::env;

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

use crate::platform;

pub fn resolve_kernel_cmd() -> anyhow::Result<String> {
  // 1) honor WOLFRAM_KERNEL_PATH
  if let Ok(raw) = env::var("WOLFRAM_KERNEL_PATH") {
    let raw = raw.trim();
    if !raw.is_empty() {
      let path = platform::shellexpand_path(raw)?;
      platform::validate_executable(&path)?;
      tracing::info!(kernel_path = %path.display(), "using kernel from WOLFRAM_KERNEL_PATH");
      return Ok(path.to_string_lossy().to_string());
    }
  }

  // 2) try platform discovery (registry on Windows, etc.)
  if let Some(p) = platform::discover_kernel_path() {
    tracing::info!(kernel_path = %p.display(), "using kernel found via platform discovery");
    return Ok(p.to_string_lossy().to_string());
  }

  // 3) fall back to PATH lookup
  for candidate in platform::get_default_kernel_names() {
    if let Some(p) = find_in_path(candidate) {
      tracing::info!(kernel_path = %p.display(), "using kernel found on PATH");
      return Ok(p.to_string_lossy().to_string());
    }
  }

  // 4) last resort: try bare WolframKernel and let OS/WSTP resolve (may work on
  //    some setups)
  tracing::warn!(
    "no kernel found via WOLFRAM_KERNEL_PATH, discovery, or PATH; trying 'WolframKernel' as a fallback"
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

pub fn evaluate(
  link: &mut Link,
  code: &str
) -> anyhow::Result<EvalResult> {
  // We wrap the code in a Module that detects graphics and returns a List:
  // {ToString[res, InputForm], If[graphicsQ[res],
  // Base64Encode[ExportByteArray[res, "PNG"]], Null]} where graphicsQ checks
  // for common graphics heads.

  let wrapper = format!(
    "ExportString[ Module[{{res, graphics}}, res = Check[{code}, $Failed]; graphics = \
     Replace[res, {{ g_ /; MemberQ[{{Graphics, Graphics3D, BoxData, Graph, GeoGraphics, Legended, \
     Placed}}, Head[g]] :> ExportString[g, \"PNG\"], _ :> Null }}]; <|\"output\" -> ToString[res, \
     InputForm], \"graphics\" -> graphics|> ], \"JSON\" ]"
  );

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

        let json_str = match result_expr.kind() {
          | ExprKind::String(s) => s.clone(),
          | _ => return Err(anyhow!("expected JSON string from kernel, got: {result_expr:?}"))
        };

        let val: serde_json::Value = serde_json::from_str(&json_str)?;
        let output = val["output"].as_str().unwrap_or("").to_string();
        let graphics = val["graphics"].as_str().map(|s| s.to_string());

        return Ok(EvalResult {
          output,
          logs,
          graphics
        });
      }
      | wstp::sys::TEXTPKT => {
        if let Ok(expr) = link.get_expr() {
          if let ExprKind::String(s) = expr.kind() {
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
  // :contentReference[oaicite:
  // 12]{index=12}

  let sym = wl_string(symbol);
  let prop = property.map(wl_string);

  let date_range = match (start_date, end_date) {
    | (Some(s), Some(e)) => {
      let s = iso_date_to_dateobject(s)?;
      let e = iso_date_to_dateobject(e)?;
      Some(format!("{{{s}, {e}}}"))
    }
    | _ => None
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

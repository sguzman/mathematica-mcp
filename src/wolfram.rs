use anyhow::{anyhow, Context};
use chrono::Datelike;
use std::path::{Path, PathBuf};
use std::{env, fs};

use wolfram_expr::{Expr, ExprKind, Symbol};
use wstp::kernel::WolframKernelProcess;
use wstp::Link;

pub fn resolve_kernel_cmd() -> anyhow::Result<String> {
    // 1) honor WOLFRAM_KERNEL_PATH
    if let Ok(raw) = env::var("WOLFRAM_KERNEL_PATH") {
        let raw = raw.trim();
        if !raw.is_empty() {
            let path = shellexpand_path(raw)?;
            validate_executable(&path)?;
            tracing::info!(kernel_path = %path.display(), "using kernel from WOLFRAM_KERNEL_PATH");
            return Ok(path.to_string_lossy().to_string());
        }
    }

    // 2) fall back to PATH lookup
    for candidate in ["WolframKernel", "MathKernel"] {
        if let Some(p) = find_in_path(candidate) {
            tracing::info!(kernel_path = %p.display(), "using kernel found on PATH");
            return Ok(p.to_string_lossy().to_string());
        }
    }

    // 3) last resort: try bare WolframKernel and let OS/WSTP resolve (may work on some setups)
    tracing::warn!(
        "no kernel found via WOLFRAM_KERNEL_PATH or PATH; trying 'WolframKernel' as a fallback"
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

pub fn eval_to_string(link: &mut Link, code: &str) -> anyhow::Result<String> {
    // Build: ToString[ReleaseHold[ToExpression[code, InputForm, HoldComplete]], InputForm]
    let to_expr = Expr::normal(
        Symbol::new("System`ToExpression"),
        vec![
            Expr::string(code),
            Expr::symbol(Symbol::new("System`InputForm")),
            Expr::symbol(Symbol::new("System`HoldComplete")),
        ],
    );
    let release = Expr::normal(Symbol::new("System`ReleaseHold"), vec![to_expr]);
    let expr = Expr::normal(
        Symbol::new("System`ToString"),
        vec![release, Expr::symbol(Symbol::new("System`InputForm"))],
    );

    link.put_eval_packet(&expr)
        .map_err(|e| anyhow!("put_eval_packet failed: {e:?}"))?;
    link.end_packet()
        .map_err(|e| anyhow!("end_packet failed: {e:?}"))?;
    link.flush().map_err(|e| anyhow!("flush failed: {e:?}"))?;

    loop {
        let pkt = link
            .raw_next_packet()
            .map_err(|e| anyhow!("raw_next_packet failed: {e:?}"))?;

        if pkt == wstp::sys::RETURNPKT {
            // ReturnPacket has one expression: the result (a string, per ToString[...] above)
            let result_expr = link
                .get_expr()
                .map_err(|e| anyhow!("get_expr failed: {e:?}"))?;

            // Discard the remainder of this packet
            link.new_packet()
                .map_err(|e| anyhow!("new_packet failed: {e:?}"))?;

            // Extract string if possible, otherwise fall back to expr formatting.
            if let ExprKind::String(s) = result_expr.kind() {
                return Ok(s.clone());
            }
            return Ok(format!("{result_expr:?}"));
        }

        // Discard non-return packets (messages, text, etc.)
        link.new_packet()
            .map_err(|e| anyhow!("new_packet failed: {e:?}"))?;
    }
}

pub fn build_financial_data_code(
    symbol: &str,
    property: Option<&str>,
    start_date: Option<&str>,
    end_date: Option<&str>,
    interval: Option<&str>,
) -> anyhow::Result<String> {
    // FinancialData forms (Wolfram docs):
    // - FinancialData["symbol"]
    // - FinancialData["symbol","property"]
    // - FinancialData["symbol","property",{start,end}]
    // (and some forms accept a time/interval argument depending on data type) :contentReference[oaicite:12]{index=12}

    let sym = wl_string(symbol);
    let prop = property.map(wl_string);

    let date_range = match (start_date, end_date) {
        (Some(s), Some(e)) => {
            let s = iso_date_to_dateobject(s)?;
            let e = iso_date_to_dateobject(e)?;
            Some(format!("{{{s}, {e}}}"))
        }
        _ => None,
    };

    let mut args = vec![format!("\"{sym}\"")];
    if let Some(p) = prop {
        args.push(format!("\"{p}\""));
    }
    if let Some(dr) = date_range {
        // If property omitted but date range provided, we still need a property.
        // Pick a reasonable default to keep syntax valid.
        if args.len() == 1 {
            args.push("\"Close\"".to_string());
        }
        args.push(dr);
    }
    if let Some(intv) = interval {
        // Optional interval argument (passed as a WL string; WL will interpret known values)
        args.push(format!("\"{}\"", wl_string(intv)));
    }

    Ok(format!("FinancialData[{}]", args.join(", ")))
}

fn iso_date_to_dateobject(s: &str) -> anyhow::Result<String> {
    // Expect YYYY-MM-DD
    let d = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .with_context(|| format!("invalid date '{s}', expected YYYY-MM-DD"))?;
    Ok(format!(
        "DateObject[{{{}, {}, {}}}]",
        d.year(),
        d.month(),
        d.day()
    ))
}

fn wl_string(s: &str) -> String {
    // Escape for inclusion inside "..."
    s.replace('\\', "\\\\").replace('\"', "\\\"")
}

fn validate_executable(path: &Path) -> anyhow::Result<()> {
    let md = fs::metadata(path).with_context(|| format!("kernel path does not exist: {path:?}"))?;
    if !md.is_file() {
        return Err(anyhow!(
            "WOLFRAM_KERNEL_PATH is not a file: {}",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = md.permissions().mode();
        if mode & 0o111 == 0 {
            return Err(anyhow!(
                "WOLFRAM_KERNEL_PATH is not executable: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn find_in_path(exe: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let cand = dir.join(exe);
        if cand.exists() {
            return Some(cand);
        }
        #[cfg(windows)]
        {
            let cand_exe = dir.join(format!("{exe}.exe"));
            if cand_exe.exists() {
                return Some(cand_exe);
            }
        }
    }
    None
}

fn shellexpand_path(raw: &str) -> anyhow::Result<PathBuf> {
    // Expand ~ and $VARS-ish cases
    let expanded = raw.replace('~', &env::var("HOME").unwrap_or_else(|_| "~".to_string()));
    Ok(PathBuf::from(expanded))
}

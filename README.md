````markdown
# mathematica-mcp-server

An MCP (Model Context Protocol) server that exposes a local **Wolfram/Mathematica kernel** over **stdio**, with:
- multi-session kernel management
- tamper-evident session IDs
- a safe-ish `FinancialData[...]` helper
- an interactive **REPL** for testing without an MCP client
- extensive structured logging via `tracing` (to stderr)

This is intended for MCP hosts like Continue and other stdio-based MCP clients.

---

## What this server provides

### Tools (all prefixed with `mathematica.`)

- `mathematica.create_session`
  - Launch a new kernel session and return a session id.
- `mathematica.execute_code`
  - Evaluate Wolfram Language code in a specific session.
- `mathematica.close_session`
  - Shutdown a specific session.
- `mathematica.list_sessions`
  - Return all active sessions and creation times.
- `mathematica.time`
  - Return current local + UTC time (RFC3339).
- `mathematica.get_finance`
  - “Sugar syntax” for `FinancialData[...]` that builds valid WL and evaluates it in-session.

---

## Requirements

### System prerequisites
- A working Wolfram kernel executable:
  - `WolframKernel` (common) or `MathKernel` (older installs)
- WSTP must be available on your system (typically installed alongside Mathematica / Wolfram Engine).
- Rust toolchain (recent stable).

### Notes about licensing / data
`FinancialData[...]` depends on your Wolfram installation and the data sources it can access. Some data requires an active license or network access.

---

## Installation & build

From the repo directory:

- Build release:
  - `cargo build --release`

The output binary will be at:
- `target/release/mathematica-mcp-server`

---

## Running

### 1) MCP server mode (stdio)
Run:
- `mathematica-mcp-server serve`

This mode is for MCP hosts. **Do not print to stdout** in this mode; MCP stdio uses stdout for protocol traffic. This project logs to **stderr** via `tracing`.

### 2) Local REPL mode (recommended for testing)
Run:
- `mathematica-mcp-server repl`

Inside the REPL you can type the same tool names you’d call from an MCP client:

Example session:
- `mathematica.create_session`
- `mathematica.execute_code 2+2`
- `mathematica.time`
- `mathematica.get_finance AAPL Close 2025-01-01 2025-02-01`
- `mathematica.list_sessions`
- `mathematica.close_session`
- `quit`

---

## Environment variables

### Required / recommended
- `ANIMALID_SECRET_KEY`
  - Secret key used to generate and verify tamper-evident session IDs.
  - If not set, a dev default is used (not recommended).

- `WOLFRAM_KERNEL_PATH`
  - Optional explicit path to the kernel executable.
  - If not set, the server will try `WolframKernel` or `MathKernel` on `PATH`.

### Logging
- `RUST_LOG`
  - Controls verbosity of tracing logs.
  - Examples: `info`, `debug`, `trace`
- `RUST_BACKTRACE=1`
  - Helpful during debugging.

### Fish examples
```text
set -x ANIMALID_SECRET_KEY "your-long-random-secret"
set -x WOLFRAM_KERNEL_PATH "/usr/local/bin/WolframKernel"
set -x RUST_LOG "info"
set -x RUST_BACKTRACE "1"
````

---

## Using with Continue (config.yaml)

Replace your Python/uv command with either a release binary (recommended) or cargo.

### Option A: Run the compiled release binary (recommended)

```yaml
- name: Mathematica (Kernel MCP)
  command: /home/admin/Code/mcp/mathematica_mcp/target/release/mathematica-mcp-server
  args:
    - serve
  env:
    ANIMALID_SECRET_KEY: "..."
    WOLFRAM_KERNEL_PATH: "/usr/local/bin/WolframKernel"
    RUST_LOG: "info"
    RUST_BACKTRACE: "1"
```

### Option B: Run via cargo (dev-friendly)

```yaml
- name: Mathematica (Kernel MCP)
  command: cargo
  cwd: /home/admin/Code/mcp/mathematica_mcp
  args:
    - run
    - --release
    - --
    - serve
  env:
    ANIMALID_SECRET_KEY: "..."
    WOLFRAM_KERNEL_PATH: "/usr/local/bin/WolframKernel"
    RUST_LOG: "info"
    RUST_BACKTRACE: "1"
```

---

## Tool details & examples

### `mathematica.create_session`

Returns:

```json
{ "session_id": "alert_fox-kind_sloth-bright_auk-calm_mole" }
```

### `mathematica.execute_code`

Input:

```json
{
  "session_id": "alert_fox-kind_sloth-bright_auk-calm_mole",
  "code": "FactorInteger[123456]"
}
```

Output:

```json
{
  "output": "{{2,6},{3,1},{643,1}}",
  "elapsed_ms": 12
}
```

### `mathematica.get_finance`

Input:

```json
{
  "session_id": "alert_fox-kind_sloth-bright_auk-calm_mole",
  "symbol": "AAPL",
  "property": "Close",
  "start_date": "2025-01-01",
  "end_date": "2025-02-01",
  "interval": "Day"
}
```

Output (example):

```json
{
  "wolfram_code": "FinancialData[\"AAPL\",\"Close\",{DateObject[{2025,1,1}], DateObject[{2025,2,1}]},\"Day\"]",
  "output": "{...}",
  "elapsed_ms": 45
}
```

Notes:

* Dates must be `YYYY-MM-DD`.
* If you provide a date range without a `property`, the server defaults the property to `"Close"` so the WL syntax remains valid.

### `mathematica.list_sessions`

Returns:

```json
{
  "sessions": [
    {
      "session_id": "alert_fox-kind_sloth-bright_auk-calm_mole",
      "created_at_utc": "2026-02-01T16:23:12Z"
    }
  ]
}
```

### `mathematica.time`

Returns:

```json
{
  "local_rfc3339": "2026-02-01T10:23:12-06:00",
  "utc_rfc3339": "2026-02-01T16:23:12Z"
}
```

---

## Architecture (how it works)

* **Session manager** maintains multiple sessions.
* Each session runs the Wolfram kernel behind a **WSTP Link**.
* Each session Link lives on its **own OS thread** (so we never require `Link: Send`).
* Tool calls communicate with session threads via channels.
* Session IDs are:

  * human-friendly (adjective_animal tokens)
  * tamper-evident via a small HMAC-derived checksum embedded in the words

---

## Troubleshooting

### “Kernel not found” / failing to launch

* Set `WOLFRAM_KERNEL_PATH` explicitly to your kernel executable.
* Verify the kernel path is executable.

### `FinancialData[...]` returns errors

* This is often due to:

  * missing data access
  * licensing limitations
  * lack of network connectivity
* Try evaluating a simpler query first:

  * `mathematica.execute_code FinancialData["AAPL"]`

### MCP host gets corrupted output / protocol errors

* Ensure the server is not printing to stdout.
* Keep logs on stderr (this project does).

### Increase logging

```text
set -x RUST_LOG "debug"
```

---

## Security notes

* Treat `ANIMALID_SECRET_KEY` like an application secret.
* Session IDs are designed to be tamper-evident, not cryptographically private.
* This server evaluates arbitrary Wolfram Language code in the kernel. Only run it in environments you trust.

---

## License

CC0-1.0 (public domain dedication).

```

If you want, paste your current repo tree (or just `Cargo.toml` + `src/` filenames), and I’ll tailor the README’s paths/commands to exactly match what you’ve actually got (binary name, tool list, and any extra tools you kept from the Python version).
::contentReference[oaicite:0]{index=0}
```

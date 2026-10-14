# Mathematica MCP

A local, Rust-based Model Context Protocol (MCP) server for Wolfram Language and Mathematica. It launches real kernels over **WSTP** and exposes computation through **stdio**. It does not send computation to a cloud service.

**Status:** v0.1.0 in development. Source-level improvements landed in October 2026, but a full native build and live Wolfram kernel run have not yet been independently verified for this maintenance pass.

## MCP tools

| Tool | Purpose |
| --- | --- |
| mathematica_create_session | Launch a kernel and issue a signed session ID |
| mathematica_execute_code | Run arbitrary Wolfram Language in the session |
| mathematica_list_sessions | Show active sessions and idle durations |
| mathematica_close_session | Shut down a session |
| mathematica_get_finance | Convenience wrapper around FinancialData |
| mathematica_time | Local and UTC time |

Each session has its own worker thread and kernel. Evaluations return InputForm text, captured print packets, elapsed milliseconds, and optional Base64-encoded PNG data.

**Security:** Code execution is deliberately arbitrary and is **not sandboxed**. Run the server only for trusted local MCP clients. Do not expose the transport to untrusted users.

## Prerequisites

- Rust toolchain and Cargo (see rust-toolchain.toml).
- A locally installed and licensed Mathematica / Wolfram Language kernel.
- A matching native WSTP development library, discoverable by the patched WSTP build script.
- Linux or Windows (both require their own native integration test).

Wolfram licensing and WSTP redistribution obligations are separate from this project's MIT license.

## Launch

For an interactive local REPL:

~~~sh
cargo build --locked
cargo run -- repl
~~~

For an MCP-compatible host, use piped stdio:

~~~sh
cargo run -- serve
~~~

The binary selects REPL automatically when launched from a terminal, and server mode when launched with piped stdin. An explicit serve command is safest in MCP configuration. Server logging goes to **stderr**, never stdout.

Typical client sequence: create a session, execute code using the returned ID, then close the session. IDs are signed with a per-process secret and **expire across server restarts**.

## Kernel path and configuration

Kernel resolution checks:

1. A platform-specific path in mathematica-mcp.toml.
2. WOLFRAM_KERNEL_PATH environment variable.
3. Wolfram installation discovery.
4. WolframKernel / MathKernel on PATH.
5. A bare WolframKernel executable as a last attempt.

The repository's TOML ships without an active absolute path. A hard-coded path would otherwise block discovery on different machines.

For an explicit path, uncomment linux_path or windows_path in mathematica-mcp.toml. For a non-default config location, set MATHEMATICA_MCP_CONFIG to the full TOML path. A missing explicitly requested configuration file is an error.

Example one-time kernel override:

~~~sh
WOLFRAM_KERNEL_PATH=/absolute/path/to/WolframKernel cargo run -- serve
~~~

## Evaluation timeouts and failure behavior

The default is 60 seconds; timeout_seconds must be between **1 and 3600**. Each evaluation uses Wolfram's native TimeConstrained, which normally aborts expensive Wolfram code without losing the session. The Rust watchdog allows an additional 10 seconds for WSTP output serialization.

If even the watchdog expires, that session is **retired** so the server cannot queue additional requests against an unresponsive kernel. Session startup is bounded to 60 seconds; shutdown waits 10 seconds for worker acknowledgement, then 5 seconds for thread termination. Idle sessions are closed after 30 minutes.

**Known native limitation:** Wolfram AbortProtect, stuck external native calls, and certain WSTP failures can prevent immediate kernel cancellation. The currently used wstp crate does not expose kernel child process termination; an unresponsive native worker may remain until process exit. The project does **not** promise guaranteed process-level kill on deadline.

## Tests and verification

Unit tests cover signed identifiers, configuration selection, evaluation-wrapper generation, and structured response parsing.

~~~sh
cargo test --locked
~~~

Run the real-kernel protocol smoke test on a machine with a working Mathematica/WSTP installation:

~~~sh
cargo build --locked
python3 scripts/mcp_smoke.py
~~~

The smoke test checks MCP initialization, tool enumeration, Wolfram arithmetic, Base64 PNG output, timeout/error behavior, session recovery, and shutdown. It uses only Python's standard library.

On GitHub-hosted CI without Wolfram native libraries, compile-time typechecking can use pregenerated FFI bindings with DOCS_RS=1. **This is not a substitute for native linking or a successful real-kernel test.**

## Source layout

| File | Responsibility |
| --- | --- |
| src/main.rs | Entrypoint and CLI dispatch |
| src/mcp.rs | MCP tool methods and JSON schemas |
| src/session.rs | Kernel workers, lifecycle and watchdog |
| src/session_id.rs | Human-readable signed session IDs |
| src/wolfram.rs | WSTP evaluation, graphics and finance helper |
| src/config.rs, src/platform/ | Kernel discovery and platform validation |
| src/repl.rs | Interactive local REPL |
| wstp-sys-patched/ | Project-specific WSTP FFI patch |
| scripts/mcp_smoke.py | End-to-end MCP regression test |

## Release state

The crate is currently **0.1.0**. Do not tag it as stable until a successful native Linux build, unit tests, MCP smoke test, and platform-specific acceptance checks. Windows requires separate native verification. See docs/reference/RELEASE.md for release gates.

License: MIT for this project's code; Wolfram and WSTP retain their separate licenses.

# Mathematica MCP

A Rust-based Model Context Protocol (MCP) server for Wolfram Language and Mathematica. It launches real kernels over **WSTP**, exposes computation through **stdio** or a **persistent multi-tenant Streamable HTTP daemon**, and does not send computation to a cloud service.

**Status:** v0.1.0 in development. **Native Linux acceptance smoke verified on 2026-10-14:** EndeavourOS with Rust 1.99.0 and Wolfram 15.0 successfully completed native WSTP build, all 10 Rust unit tests, strict Clippy, and the real-kernel MCP smoke suite (including PNG graphics, kernel-side timeout/recovery and clean shutdown). This is an observed single-machine acceptance run, not a stability certification for every platform or failure mode.

## Shared systemd daemon (LAN)

The `serve-http` command listens on a configurable local/LAN address. **One
systemd process serves all MCP clients**; every MCP client receives an independent
handler and may create its own Wolfram kernel sessions. The shared manager limits
the total number of kernels across agents. Agents cannot list, execute, or close
each other's kernel sessions. Existing stdio and embedded Rust entrypoints remain
supported.

On your EndeavourOS machine, the native-test-first **one-command installer**
is:

```sh
python3 scripts/deploy_user_service.py
```

It preserves your existing external TOML config and enables/restarts the
systemd user service only after native acceptance passes. For manual
deployment instead:

```sh
cargo build --locked --release
install -Dm755 target/release/mathematica-mcp ~/.local/bin/mathematica-mcp
install -Dm644 deploy/mathematica-mcp.service ~/.config/systemd/user/mathematica-mcp.service
mkdir -p ~/.config/mathematica-mcp
cp deploy/config.example.toml ~/.config/mathematica-mcp/config.toml
# Edit [server].listen to an IP address on this machine (or 0.0.0.0).
systemctl --user daemon-reload
systemctl --user enable --now mathematica-mcp.service
```

Remote agents point their Streamable HTTP MCP client to
`http://HOST_LAN_IP:8765/mcp`. The service's health endpoint is
`http://HOST_LAN_IP:8765/healthz`. The daemon binds only to the configured
`[server].listen` address. `[tenancy].max_kernels` bounds the number of
*running or still-blocked* worker threads; idle kernel sessions expire after
30 minutes. HTTP MCP sessions are managed separately from Wolfram kernel
sessions: use `mathematica_create_session` before evaluating. Restarting the
daemon invalidates old MCP/Wolfram sessions.

**Danger:** Arbitrary Wolfram Language can execute OS commands under the
systemd service account. Exposing the service without authentication to a LAN
gives anyone who can reach that port arbitrary code execution as that account.
Only bind on networks and hosts you fully trust; restrict network access if
other people/devices share the LAN. This deployment deliberately does **not**
implement user authentication or TLS.

See [daemon configuration, host setup, systemd commands, and troubleshooting](docs/reference/DAEMON.md).
A real-Wolfram HTTP tenant-isolation test is included in `scripts/http_smoke.py`.

## Embed in other Rust projects

**Rust library support:** the package exports a native, async library named `mathematica_mcp`, in addition to its existing `mathematica-mcp` executable. Embedded callers do not need the MCP JSON-RPC protocol or spawn the executable.

Use a Git dependency in a different Rust project's `Cargo.toml` (the crate has **not** been published to crates.io):

~~~toml
[dependencies]
mathematica-mcp = { git = "https://github.com/sguzman/mathematica-mcp" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
anyhow = "1"
~~~

Use the library inside an existing Tokio runtime:

~~~rust
use std::time::Duration;
use mathematica_mcp::Mathematica;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let wolfram = Mathematica::new()?;
    let session = wolfram.create_session().await?;
    let answer = wolfram.execute(&session, "40 + 2", Duration::from_secs(20)).await?;
    println!("{}", answer.output); // 42
    wolfram.close_session(&session).await?;
    Ok(())
}
~~~

**Public API:** `Mathematica::new`, `create_session`, `execute`, `get_finance` (using `FinanceQuery`), `list_sessions`, `close_session`. The returned `EvalResult` contains Wolfram output, captured logs, and optional Base64 PNG graphics. All library calls use native WSTP, not the MCP server or a subprocess shell wrapper.

**Deployment requirements:** the consuming machine needs an installed, licensed Wolfram kernel and native WSTP development library. A running Tokio runtime is required. Wolfram code executes as the calling OS user and is not sandboxed. The repository uses a patched WSTP FFI crate during its own build; Cargo does not inherit another dependency's `[patch.crates-io]` table. The independent consumer test intentionally validates a fresh downstream dependency resolution, and its native run is an acceptance gate.

The self-contained consumer fixture is in `tests/consumer`. The library also ships `examples/embedded.rs`. The native Linux QA runner tests both the MCP server and the independent consuming binary against the installed kernel. The library has not been certified for Windows or published to crates.io.

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

- Your existing compatible Rust toolchain and Cargo (Rust 2024 edition; no repository-pinned nightly or automatic project-specific compiler installation).
- A locally installed and licensed Mathematica / Wolfram Language kernel.
- A matching native WSTP development library, discoverable by the patched WSTP build script.
- Linux is the only release target for this QA cycle. Legacy Windows support remains in source but is not part of the acceptance gate.

Wolfram licensing and WSTP redistribution obligations are separate from this project's MIT license.

## Launch

The repository deliberately **does not contain a `rust-toolchain.toml`**. Cargo and rustfmt use your existing toolchain and default Rustup selection. If you have explicitly selected another toolchain using `RUSTUP_TOOLCHAIN` or a directory override, Rustup respects that separate setting.

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

## Wolfram 15 PNG export prerequisite

On Wolfram 15.0, graphics PNG export can trigger a first-use download of WolframResearch's `ImageMetadataTools` paclet from GitHub. On slow or offline systems this appears to hang the kernel even though arithmetic and symbolic plotting work.

**Ordinary builds and MCP requests do not install this package.** Graphics requests fail with an explicit missing-paclet error rather than silently attempting a network download. The separate Linux graphics probe also checks for the paclet before trying an export.

Check installed state without downloads or writes:

~~~sh
python3 -u scripts/install_image_metadata_tools.py --check
~~~

If absent, install the official WolframResearch 1.0.0 paclet **once, explicitly**:

~~~sh
python3 -u scripts/install_image_metadata_tools.py --install
~~~

The installer downloads the **6.9 MiB** official release to the user's persistent XDG cache, resumes interrupted transfers, prints progress, validates the official SHA-256 checksum, and calls Wolfram `PacletInstall` on the verified local file. A successful installation persists in Wolfram's paclet directory and does not need repeating for every plot. No repository clone or Rust toolchain download is involved.

## Tests and verification

Unit tests cover signed identifiers, configuration selection, evaluation-wrapper generation, and structured response parsing.

~~~sh
cargo test --locked
~~~

Run the complete Linux validation entrypoint (native compile, tests, Clippy, then live MCP protocol smoke) from a Linux machine with working Mathematica/WSTP:

~~~sh
fish scripts/linux_qa.fish
~~~

For the kernel-independent MCP test-client tests, use:

~~~sh
python3 -m unittest discover -s scripts -p 'test_*.py'
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

The crate is currently **0.1.0**. The new Streamable HTTP/systemd
deployment has source-level acceptance checks; its real-kernel, concurrent LAN
acceptance must be rerun on the target machine before treating it as deployed.
The older native smoke result below predates the HTTP additions. The initial Linux native build, unit-test, lint, and real-kernel smoke gates passed on 2026-10-14 (Jobflick run 89582e89-ed7a-4121-94b8-5c1ce9bcce81, revision 8728455); stdout JSON-RPC isolation, exceptional native process cleanup and missing-config edge cases still require dedicated acceptance checks before declaring a fully certified release. Windows is preserved as legacy code but is not a release gate here. See docs/reference/RELEASE.md for remaining gates.

License: MIT for this project's code; Wolfram and WSTP retain their separate licenses.

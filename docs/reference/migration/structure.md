# Project lineage and structure

The original Python implementation is available at:
https://github.com/sguzman/mathematica_mcp

The Rust successor is this repository, sguzman/mathematica-mcp.

## Essential modules

- src/mcp.rs: typed MCP tools with rmcp and stdio transport
- src/session.rs: stateful per-session WSTP worker and cleanup management
- src/session_id.rs: ephemeral HMAC-signed readable IDs
- src/wolfram.rs: kernel discovery, evaluation, JSON and graphics wrappers
- src/config.rs and src/platform/: kernel path discovery
- src/repl.rs: interactive development shell
- wstp-sys-patched/: patched WSTP FFI build boundary

Python parity and runtime interoperability should be demonstrated with real-kernel tests, not inferred from equivalent-looking function names. See scripts/mcp_smoke.py for the current regression scenarios.

This replaces an unused generic migration template; no fictional reference project is required.

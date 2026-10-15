# Changelog

All notable changes to Mathematica MCP are recorded here. Semantic Versioning
is used from v1.0.0.

## [1.0.0] - 2026-10-15

### First stable Linux MVP

- Real Wolfram kernel execution over native WSTP, signed ephemeral session
  identifiers and bounded evaluation/timeout handling.
- MCP stdio and multi-tenant Streamable HTTP transports.
- Per-agent Wolfram session ownership, two-agent kernel isolation, shared
  configurable kernel count and idle session cleanup.
- Mathematica code execution, printed logs, PNG graphics, finance helper
  and current time helper.
- Public native Rust library, real-kernel independent consumer acceptance.
- TOML kernel and bind address configuration, systemd user unit, automatic
  native-test-first deployment script.
- Native Wolfram 15 WSTP, stdio and two-agent HTTP tests; release source CI,
  documentation and Linux operating instructions.

### Known limitations

- Uncancellable native WSTP calls are not guaranteed to be forcibly killed.
- No built-in HTTP authentication/TLS. Trusted networks only.
- No verification from a second host over the LAN in the acceptance log.
- Windows is unverified; crates.io and proprietary Wolfram binaries excluded.

[1.0.0]: https://github.com/sguzman/mathematica-mcp/releases/tag/v1.0.0

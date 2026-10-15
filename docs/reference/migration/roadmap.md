# Mathematica MCP stabilization history

**Completed — stable v1.0.0 Linux MVP (2026-10-15).**
This is the completed migration record, not an outstanding release gate.

## Completed milestones

- Replaced format-only session IDs with ephemeral signed IDs.
- Added Wolfram `TimeConstrained`, watchdogs, idle cleanup, and
  rejection of concurrent evaluations on one kernel.
- Verified Base64 PNG graphics, finance-data helper, and error responses.
- Removed obsolete hard-coded Wolfram kernel paths.
- Implemented real-kernel stdio MCP acceptance and strict CI source checks.
- Exported a native Rust library and ran the independent consumer.
- Implemented centralized multi-client Streamable HTTP MCP with per-agent
  Wolfram kernel session ownership, shared kernel cap and TOML config.
- Installed and verified an enabled systemd user service on EndeavourOS.
- Passed native WSTP, two-agent HTTP, Rust library, stdio and timeout tests.
- Released stable Linux v1.0.0 (source-only).

## Post-release improvements (non-blocking)

- Investigate stuck WSTP native call and child-process cleanup (issue #1).
- Test access from a different LAN machine (issue #2).
- Extra stdout/config and SIGTERM exceptional-path checks (issues #1/#2).
- Windows verification deferred; not part of Linux v1.0.0.

See [release policy](../RELEASE.md) for support and limitations.

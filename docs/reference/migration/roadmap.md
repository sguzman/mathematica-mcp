# Mathematica MCP stabilization roadmap

The Rust server is implemented but not yet certified as a stable release.

## Implemented in the October 2026 maintenance pass

- Replace format-only session IDs with ephemeral signed IDs.
- Apply TimeConstrained inside Wolfram and bound Rust watchdog waits.
- Retire unresponsive sessions, bound worker queues and avoid idle reaping of active calls.
- Encode PNG graphics with BaseEncode and test JSON response parsing.
- Remove stale absolute kernel override paths; diagnose missing explicit config.
- Add a real-kernel stdio MCP protocol smoke test and kernel-independent source checks.
- Replace outdated documentation and maintenance templates.

## Remaining release gates

- Obtain a clean native WSTP build on Linux and pass cargo test.
- Run scripts/mcp_smoke.py against a licensed local kernel, review logs, and fix any failures.
- Verify no orphaned child processes during a stuck native WSTP call. Current library exposes no direct kill method.
- Verify Python-to-Rust behavior parity where it matters for existing consumers.
- Run the same native acceptance tests on Windows.
- Tag only after the acceptance matrix is recorded in the release notes.

Until these checks pass, this repository remains a **development** release at v0.1.0.

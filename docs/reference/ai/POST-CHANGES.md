# Post-change verification

Validation depends on whether Wolfram/WSTP is installed.

## Kernel-independent checks

- cargo metadata --locked --no-deps
- DOCS_RS=1 cargo check --locked --all-targets (typecheck only; this is not native linking)
- python3 -m py_compile scripts/mcp_smoke.py
- cargo fmt --check, using the installed stable rustfmt component

## Real native verification (required before a release)

- fish scripts/linux_qa.fish (build, cargo test, clippy, live MCP smoke)
- Linux is the only required acceptance platform for this maintenance cycle.

Do not claim these checks passed merely because the commands are documented. Record actual results in the release notes. No nonexistent post-change scripts or benchmarks are required.

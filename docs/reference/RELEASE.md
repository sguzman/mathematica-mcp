# Release and maintenance policy

Current Cargo version: **0.1.0**. Changes on main are development work, not a certified release.

## Acceptance gates

1. Rust formatting, clippy, and the project test suite pass with the pinned toolchain.
2. A **native Linux WSTP build** completes against an installed Wolfram kernel.
3. The unit tests pass on the same native machine.
4. The real-kernel test in scripts/mcp_smoke.py passes: MCP handshake, arithmetic, PNG, timeout, recovery and shutdown.
5. Verify configuration discovery, explicitly missing config errors and clean stdout JSON-RPC.
6. Repeat a native build and smoke test on Windows before claiming Windows release parity.

GitHub CI may typecheck without a kernel using generated bindings, but such a check does not prove native WSTP linkage or runtime functionality.

## Version policy

- Use Conventional Commits.
- Stay in 0.x until runtime, protocol, and configuration semantics stabilize.
- A signed-session-ID format is incompatible with the original format; session IDs are intentionally valid only during the owning process.
- Do not tag 1.0.0 until native integration tests pass on all advertised platforms and the supported contract is documented.

## Publishing

Only after the gates succeed: update Cargo.toml, write the change log, create an annotated tag for the verified source, and publish artifacts only where Wolfram licensing permits it. Avoid distributing proprietary SDK binaries beyond the terms of the relevant Wolfram agreement.

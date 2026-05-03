# Mathematica MCP

Mathematica MCP is a Rust MCP server that exposes a local Wolfram/Mathematica kernel over stdio.

## Intent

Make a local Mathematica kernel accessible to MCP-compatible clients while preserving session control, observability, and a local REPL for direct testing.

## Ambition

The session-management, REPL, and helper tooling indicate an ambition to be a dependable bridge between LLM tooling and a local symbolic-computation environment.

## Current Status

The server, session management, REPL, docs, and environment-variable surface are already implemented and documented. This looks like an actively usable integration project.

## Core Capabilities Or Focus Areas

- Serve Mathematica/Wolfram functionality over MCP stdio transport.
- Manage multiple kernel sessions.
- Expose a local REPL mode for testing.
- Provide helper functionality around safe-ish kernel operations.
- Use tracing and structured diagnostics for operational visibility.

## Project Layout

- `docs/`: project documentation, reference material, and roadmap notes.
- `src/`: Rust source for the main crate or application entrypoint.
- `Cargo.toml`: crate or workspace manifest and the first place to check for package structure.

## Setup And Requirements

- Rust toolchain.
- A working Wolfram/Mathematica installation with kernel access.
- An MCP host or local REPL workflow.

## Build / Run / Test Commands

```bash
cargo build
cargo test
cargo run -- --help
```

## Notes, Limitations, Or Known Gaps

- This project depends on local proprietary tooling, so setup is a meaningful part of the runtime story.
- The boundary between MCP transport and kernel execution is a core reliability concern.

## Next Steps Or Roadmap Hints

- Keep kernel/session lifecycle rules explicit as more tools are exposed.
- Add more parity and operational tests around long-lived session behavior if needed.

# Maintainer rules

Maintain this as a standalone Rust application with a small, auditable MCP surface.

- Implement coherent changes directly in the repository when authorized by the owner. Prefer direct commits to unnecessary PR traffic.
- Use Conventional Commits and accurately state any compatibility changes.
- Do not claim a successful build without a build. A GitHub-hosted typecheck is not a native WSTP integration test.
- Keep session, timeout and shutdown behavior explicit. Never block the async runtime on synchronous kernel I/O.
- Treat arbitrary Wolfram Language execution as trusted local code, not as a sandbox.
- Preserve stderr-only diagnostics in server mode; stdout is reserved for MCP JSON-RPC.
- Keep machine-specific paths, credential material and private venting out of public commits.
- When practical, add unit tests and a real-kernel regression case for behavior changes.
- Do not pin a nightly Rust toolchain or implicitly install compilers on local developer machines. Use stable Rust and the installed default toolchain.
- Avoid unnecessary dependencies and cross-platform behavior regressions.
- Split Rust modules by responsibility before they become unmanageably large (roughly 600 lines).

# Post-change verification

Validation depends on whether Wolfram/WSTP is installed.

## Kernel-independent checks

- cargo metadata --locked --no-deps
- DOCS_RS=1 cargo check --locked --all-targets (typecheck only; this is not native linking)
- python3 -m py_compile scripts/mcp_smoke.py
- cargo fmt --check, where the repository's nightly rustfmt toolchain is available

## Real native verification (required before a release)

- cargo build --locked
- cargo test --locked
- cargo clippy --all-targets -- -D warnings
- python3 scripts/mcp_smoke.py
- Repeat platform-specific native smoke tests before advertising that platform as verified.

Do not claim these checks passed merely because the commands are documented. Record actual results in the release notes. No nonexistent post-change scripts or benchmarks are required.

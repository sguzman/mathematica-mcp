# Release and maintenance policy

## v1.0.0: stable Linux MVP (2026-10-15)

The intended workflows are complete: stdio MCP, a centralized multi-tenant
Streamable HTTP service, per-agent Wolfram kernel sessions, public native Rust
library, external TOML, and a persistent user-systemd deployment.

### Native acceptance recorded

On EndeavourOS with Rust 1.99.0 and Wolfram 15.0, native acceptance of
revision `c20fd0c` completed successfully on 2026-10-15:

- WSTP native build/link, 12 Rust unit tests, strict Clippy.
- Stdio MCP with live Mathematica kernel: handshake, arithmetic, PNG graphics,
  timeout/recovery, and normal shutdown.
- HTTP MCP: two simultaneous independently initialized agents; independent
  kernel variables; per-agent ownership and foreign-session rejection;
  shared kernel limit; normal kernel shutdown.
- Separately resolved downstream Rust library consumer running a real kernel.
- Release binary build, systemd user-service enablement/start and successful
  local HTTP health probe.

GitHub source checks passed before the release candidate:
https://github.com/sguzman/mathematica-mcp/actions/runs/38809697983

GitHub-hosted CI without licensed Wolfram only typechecks and lints; the
native evidence above comes from a real Wolfram-equipped Linux machine.

## v1 stable compatibility scope

- Documented MCP tools/schemas and `serve` stdio transport.
- `serve-http` Streamable HTTP server and per-transport-client Wolfram
  session scope; `mathematica_create_session` is called separately.
- Signed Wolfram session IDs are ephemeral, invalid after restart; no
  persistent state contract.
- TOML `[server]`, `[tenancy]`, `[kernel]` and the
  `MATHEMATICA_MCP_CONFIG` environment variable.
- Public Rust `mathematica_mcp` library on Linux.
- Configurable shared kernel concurrency, timeouts and cleanup behavior.
- User systemd unit and release deployment workflow.

Use SemVer: patch for compatible fixes, minor for compatible additions,
major for breaking interface or configuration changes. A stable contract
does **not** imply feature completeness beyond the documented v1 scope.

## Explicit known limitations and post-release work

- Uncancellable native WSTP calls may leave processes after watchdog
  retirement; no guaranteed OS-level force-kill (issue #1).
- An actual remote *second machine* on the LAN has not been verified,
  although localhost HTTP and two-client kernel isolation passed (issue #2).
- No HTTP authentication or TLS: Wolfram code may run OS commands under the
  service account. Only bind on fully trusted networks.
- stdout purity, missing-config exceptional cases, and SIGTERM orphan
  process audits have not been exhaustively tested (issues #1 and #2).
- Legacy Windows remains unverified and is not the release target.

These are honest maintenance limitations, not v1.0.0 release blockers.

## Publishing

- Use Conventional Commits and SemVer.
- A first-party CI release job, after the source acceptance checks, creates
  an annotated Git tag and a GitHub source release from the release commit.
- **Source release only**: Wolfram and native WSTP have separate licenses;
  do not bundle proprietary components. No crates.io publication, no
  universal prebuilt binaries.
- A GitHub tag or release never updates the running user's systemd binary.
  Redeploy locally from the tagged source with
  `python3 scripts/deploy_user_service.py`.

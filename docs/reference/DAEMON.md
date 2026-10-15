# Persistent LAN MCP daemon

One `mathematica-mcp serve-http` process runs under systemd and serves multiple
agents over MCP **Streamable HTTP**. Its Wolfram engine is shared; sessions and
state are isolated by MCP client/transport session.

## Install on EndeavourOS / Arch Linux

Use the licensed Wolfram 15 installation and native WSTP SDK already available
to the normal user. Run the native QA before deploying a new binary.

```sh
cargo build --locked --release
install -Dm755 target/release/mathematica-mcp ~/.local/bin/mathematica-mcp
install -Dm644 deploy/mathematica-mcp.service ~/.config/systemd/user/mathematica-mcp.service
mkdir -p ~/.config/mathematica-mcp
cp deploy/config.example.toml ~/.config/mathematica-mcp/config.toml
```

Adjust `~/.config/mathematica-mcp/config.toml`, especially `[server].listen`:
- `127.0.0.1:8765` local machine only.
- `192.168.1.50:8765` only that *assigned* host LAN address.
- `0.0.0.0:8765` all IPv4 interfaces.
- `[::]:8765` all IPv6 interfaces (where supported).

`[server].endpoint` defaults to `/mcp`; it must begin with a slash,
must not be `/`, and must not end with a slash.
`[tenancy].max_kernels` defaults to 4 (valid range 1..=128). It is
a shared upper bound, not a per-agent cap. Opening a new kernel above the
limit returns a normal tool error until an existing worker terminates.

Service installation:

```sh
systemctl --user daemon-reload
systemctl --user enable --now mathematica-mcp.service
systemctl --user status mathematica-mcp.service
journalctl --user -u mathematica-mcp.service -f
curl http://127.0.0.1:8765/healthz
```

The health endpoint answers even when Wolfram has not launched; it verifies
only that the HTTP process is accepting requests, **not** native WSTP health.
For service auto-start *without logging in*, an administrator can run
`loginctl enable-linger USERNAME` once, where appropriate. This is a user
unit: do not run the Wolfram daemon as root.

Agents need **no executable** and **no Mathematica installation** on their
machines. Configure the agent's MCP Streamable HTTP URL:
`http://HOST_LAN_IP:8765/mcp`. Open the host OS firewall for TCP port 8765
if necessary. Every agent initializes an independent MCP transport session,
then calls `mathematica_create_session` to allocate a Wolfram kernel.
Subsequent evaluations must carry that Wolfram session ID. Another agent's
MCP transport session cannot enumerate or use it. Closing a Wolfram session
frees its worker, and idle sessions expire after 30 minutes. A daemon restart
invalidates all IDs. Sessions do not survive reconnects that create new MCP
transport sessions.

The server remains compatible with `mathematica-mcp serve` (stdio) and the
direct Rust embedding API. Its HTTP client session state is **in-memory**.
`mathematica-mcp repl` remains interactive; the systemd unit uses
`serve-http` explicitly. Logging goes to systemd journal / stderr.

## Testing

```sh
fish scripts/linux_qa.fish
```

The runner builds and runs both stdio and HTTP native MCP smoke tests, plus
the independent Rust library consumer. The HTTP smoke probes a health
endpoint, initializes two transport sessions, evaluates real Wolfram code,
tests cross-agent rejection, and closes the kernel.

## Security boundary

**This service intentionally has no built-in authentication or TLS.** Wolfram
is not sandboxed: `RunProcess`, `Run`, file operations, and native integration
execute using the service account's OS permissions. Anyone on a routable
network allowed to reach this endpoint gains that effective privilege.
The signed Wolfram session IDs and per-MCP-client session scoping prevent
accidental agent state mixing; they are not access-control protection against
arbitrary remote code execution. Only expose the port to trusted clients.

## Troubleshooting

- `Address not available`: the configured address is not assigned to this
  machine; use its actual LAN IP or `0.0.0.0`.
- `Connection refused`: service not running, wrong port/interface, or firewall.
- `kernel limit reached`: agents should close unused Wolfram sessions;
  idle cleanup also retires inactive ones.
- `session not found for this MCP client`: the client restarted its MCP
  transport session; call `mathematica_create_session` again.
- Wolfram/WSTP discovery issues: set `[kernel].linux_path` or
  `WOLFRAM_KERNEL_PATH` in the service environment. See README for
  native Wolfram 15 PNG paclet requirements.

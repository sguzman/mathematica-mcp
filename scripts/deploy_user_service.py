#!/usr/bin/env python3
"""Accept, install, and activate the Mathematica MCP user daemon on Linux.

Run from any working directory:
    python3 scripts/deploy_user_service.py

Requires a local licensed Wolfram kernel, WSTP SDK, Rust, Fish, and systemd.
A failed native acceptance test stops deployment before changing the service.
"""
from __future__ import annotations

import http.client
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib


ROOT = Path(__file__).resolve().parents[1]
HOME = Path.home()
SERVICE = "mathematica-mcp.service"
CONFIG = HOME / ".config/mathematica-mcp/config.toml"
SERVICE_FILE = HOME / ".config/systemd/user" / SERVICE
BIN = HOME / ".local/bin/mathematica-mcp"


def command(*args: str) -> None:
    print("+", " ".join(args), flush=True)
    subprocess.run(args, cwd=ROOT, check=True)


def atomic_copy(source: Path, destination: Path, mode: int) -> None:
    """Replace the executable atomically, including while an old daemon runs."""
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as handle:
        staged = Path(handle.name)
    try:
        shutil.copyfile(source, staged)
        staged.chmod(mode)
        os.replace(staged, destination)
    finally:
        staged.unlink(missing_ok=True)


def listener(config_path: Path) -> tuple[str, int, str]:
    settings = tomllib.loads(config_path.read_text(encoding="utf-8"))
    server = settings.get("server", {})
    address = server.get("listen", "127.0.0.1:8765")
    endpoint = server.get("endpoint", "/mcp")
    if not isinstance(address, str) or ":" not in address:
        raise ValueError(f"invalid listener in {config_path}: {address!r}")
    host, port_text = address.rsplit(":", 1)
    if host.startswith("[") and host.endswith("]"):
        host = host[1:-1]
    port = int(port_text)
    if not 1 <= port <= 65535 or not host:
        raise ValueError(f"invalid listener in {config_path}: {address!r}")
    if not isinstance(endpoint, str) or not endpoint.startswith("/"):
        raise ValueError(f"invalid MCP endpoint in {config_path}")
    return host, port, endpoint


def probe_health(host: str, port: int) -> bool:
    if host == "0.0.0.0":
        host = "127.0.0.1"
    elif host == "::":
        host = "::1"
    try:
        connection = http.client.HTTPConnection(host, port, timeout=2)
        try:
            connection.request("GET", "/healthz")
            response = connection.getresponse()
            return response.status == 200 and response.read() == b"ok"
        finally:
            connection.close()
    except (OSError, TimeoutError):
        return False


def main() -> int:
    if platform.system() != "Linux" or os.geteuid() == 0:
        raise RuntimeError("Run as your normal Linux desktop user, not root")
    for executable in ("cargo", "fish", "systemctl"):
        if not shutil.which(executable):
            raise RuntimeError(f"missing prerequisite: {executable}")

    print("=== Native acceptance (no installation until every test passes) ===", flush=True)
    command("fish", "scripts/linux_qa.fish")
    command("cargo", "build", "--locked", "--release")

    print("=== Installing user service ===", flush=True)
    if not CONFIG.exists():
        atomic_copy(ROOT / "deploy/config.example.toml", CONFIG, 0o600)
        print(f"Created LAN configuration: {CONFIG}", flush=True)
    else:
        print(f"Preserving existing configuration: {CONFIG}", flush=True)
    host, port, endpoint = listener(CONFIG)

    atomic_copy(ROOT / "target/release/mathematica-mcp", BIN, 0o755)
    if SERVICE_FILE.exists() and SERVICE_FILE.read_bytes() != (
        ROOT / "deploy/mathematica-mcp.service"
    ).read_bytes():
        backup = SERVICE_FILE.with_name(SERVICE + ".previous")
        shutil.copy2(SERVICE_FILE, backup)
        print(f"Previous service unit saved to: {backup}", flush=True)
    atomic_copy(ROOT / "deploy/mathematica-mcp.service", SERVICE_FILE, 0o644)

    command("systemctl", "--user", "daemon-reload")
    command("systemctl", "--user", "enable", SERVICE)
    command("systemctl", "--user", "restart", SERVICE)
    command("systemctl", "--user", "is-active", "--quiet", SERVICE)

    for _ in range(30):
        if probe_health(host, port):
            break
        time.sleep(0.4)
    else:
        raise RuntimeError(
            "service started but HTTP health probe failed; "
            "inspect journalctl --user -u mathematica-mcp.service"
        )

    print("PASS: Mathematica MCP daemon installed, active, and answering HTTP", flush=True)
    if host in ("0.0.0.0", "::"):
        print(f"Agent URL: http://<HOST_LAN_IP>:{port}{endpoint}", flush=True)
    else:
        bracketed = f"[{host}]" if ":" in host else host
        print(f"Agent URL: http://{bracketed}:{port}{endpoint}", flush=True)

    if shutil.which("loginctl"):
        result = subprocess.run(
            ("loginctl", "show-user", str(os.getuid()), "-p", "Linger", "--value"),
            capture_output=True, text=True, check=False,
        )
        if result.returncode == 0 and result.stdout.strip().lower() != "yes":
            print(
                "NOTE: systemd linger is disabled; the user service may stop "
                "when the last login ends. Enable linger separately if needed.",
                flush=True,
            )
    print("Remaining acceptance: connect a second device across your LAN.", flush=True)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as exc:
        print(f"DEPLOYMENT FAILED: {exc}", file=sys.stderr, flush=True)
        sys.exit(1)

#!/usr/bin/env python3
"""Native multi-agent HTTP acceptance: requires installed Wolfram/WSTP.

The smoke starts a temporary loopback Streamable HTTP daemon, establishes
two independent MCP transport sessions, and proves kernel session isolation.
Uses only the Python standard library.
"""
from __future__ import annotations

import http.client
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time

def read_sse_jsonrpc(response: http.client.HTTPResponse, request_id: object) -> dict:
    """Read a matching JSON-RPC response from an SSE stream.

    SSE priming/retry events and keep-alives can contain no JSON payload.
    Accumulate data lines until the blank line terminating an SSE event,
    ignoring control events, notifications, and unrelated request IDs.
    """
    data_lines: list[bytes] = []
    while True:
        line = response.readline()
        if not line:
            raise RuntimeError("SSE stream ended without JSON-RPC response")
        line = line.rstrip(b"\r\n")
        if line:
            if line.startswith(b":"):
                continue  # SSE comment/keep-alive
            field, sep, value = line.partition(b":")
            if field == b"data" and sep:
                # SSE removes at most one leading ASCII space after the colon.
                if value.startswith(b" "):
                    value = value[1:]
                data_lines.append(value)
            continue

        if not data_lines:
            continue  # SSE control-only event (id, retry, comment, etc.)
        raw = b"\n".join(data_lines)
        data_lines.clear()
        if not raw.strip():
            continue  # Valid empty SSE priming event; no JSON to decode.
        try:
            message = json.loads(raw)
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise RuntimeError(f"invalid JSON-RPC SSE event: {raw[:160]!r}") from exc
        if isinstance(message, dict) and message.get("id") == request_id:
            return message



class HttpMcpClient:
    def __init__(self, host: str, port: int) -> None:
        self.host = host
        self.port = port
        self.session_id: str | None = None
        self.request_id = 0

    def call(self, method: str, params: dict, timeout: float = 90) -> dict:
        self.request_id += 1
        return self._post({
            "jsonrpc": "2.0", "id": self.request_id,
            "method": method, "params": params,
        }, timeout=timeout)

    def notify(self, method: str) -> None:
        self._post({"jsonrpc": "2.0", "method": method}, timeout=15)

    def _post(self, payload: dict, timeout: float) -> dict:
        connection = http.client.HTTPConnection(self.host, self.port, timeout=timeout)
        headers = {
            "Accept": "application/json, text/event-stream",
            "Content-Type": "application/json",
        }
        if self.session_id:
            headers["Mcp-Session-Id"] = self.session_id
        try:
            connection.request("POST", "/mcp", body=json.dumps(payload), headers=headers)
            response = connection.getresponse()
            if response.getheader("Mcp-Session-Id"):
                self.session_id = response.getheader("Mcp-Session-Id")
            if response.status == 202:
                response.read()
                return {}
            if response.status >= 400:
                raise RuntimeError(f"MCP HTTP {response.status}: {response.read(4096)!r}")
            mime = response.getheader("Content-Type", "")
            if "application/json" in mime:
                return json.loads(response.read())
            if "text/event-stream" not in mime:
                raise RuntimeError(f"unexpected response Content-Type: {mime}")
            return read_sse_jsonrpc(response, payload.get("id"))
        finally:
            connection.close()

    def initialize(self, name: str) -> None:
        result = self.call("initialize", {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": name, "version": "1.0"},
        })
        assert "result" in result, f"MCP initialize failed: {result}"
        assert self.session_id, "stateful MCP transport did not provide Mcp-Session-Id"
        self.notify("notifications/initialized")

    def tool(self, name: str, **kwargs: object) -> dict:
        result = self.call("tools/call", {"name": name, "arguments": kwargs})
        if "error" in result:
            raise RuntimeError(f"{name}: {result['error']}")
        value = result["result"]
        if value.get("isError"):
            raise RuntimeError(f"{name}: {value}")
        if isinstance(value.get("structuredContent"), dict):
            return value["structuredContent"]
        for part in value.get("content", []):
            if part.get("type") == "text":
                content = json.loads(part["text"])
                if isinstance(content, dict):
                    return content
        raise RuntimeError(f"{name}: missing structured content: {value}")


def ephemeral_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def wait_for_health(proc: subprocess.Popen, port: int) -> None:
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"HTTP daemon exited with status {proc.returncode}")
        try:
            connection = http.client.HTTPConnection("127.0.0.1", port, timeout=1)
            try:
                connection.request("GET", "/healthz")
                response = connection.getresponse()
                if response.status == 200 and response.read() == b"ok":
                    return
            finally:
                connection.close()
        except (OSError, TimeoutError):
            pass
        time.sleep(0.2)
    raise TimeoutError("HTTP daemon health endpoint did not start")


def run(executable: str) -> None:
    port = ephemeral_port()
    with tempfile.TemporaryDirectory(prefix="mathematica-http-smoke-") as tmp:
        config = Path(tmp) / "config.toml"
        config.write_text(
            f'[server]\nlisten = "127.0.0.1:{port}"\nendpoint = "/mcp"\n'
            '[tenancy]\nmax_kernels = 2\n',
            encoding="utf-8",
        )
        env = os.environ.copy()
        env["MATHEMATICA_MCP_CONFIG"] = str(config)
        proc = subprocess.Popen([executable, "serve-http"], env=env)
        first = HttpMcpClient("127.0.0.1", port)
        second = HttpMcpClient("127.0.0.1", port)
        wolfram_id: str | None = None
        second_id: str | None = None
        try:
            wait_for_health(proc, port)
            print("PASS: HTTP daemon serves health endpoint", flush=True)
            first.initialize("smoke-agent-one")
            second.initialize("smoke-agent-two")
            assert first.session_id != second.session_id
            print("PASS: independent MCP transport sessions", flush=True)

            listed = first.call("tools/list", {})["result"]["tools"]
            assert any(item["name"] == "mathematica_create_session" for item in listed)
            wolfram_id = first.tool("mathematica_create_session")["session_id"]
            output = first.tool(
                "mathematica_execute_code", session_id=wolfram_id,
                code="6*7", timeout_seconds=20,
            )
            assert output["output"] == "42", output
            print("PASS: HTTP tool dispatch to real Wolfram kernel", flush=True)

            second_id = second.tool("mathematica_create_session")["session_id"]
            assert second_id != wolfram_id
            first.tool("mathematica_execute_code",
                       session_id=wolfram_id, code="tenantValue = 41")
            second.tool("mathematica_execute_code",
                        session_id=second_id, code="tenantValue = 99")
            assert first.tool("mathematica_execute_code",
                              session_id=wolfram_id, code="tenantValue")["output"] == "41"
            assert second.tool("mathematica_execute_code",
                               session_id=second_id, code="tenantValue")["output"] == "99"
            print("PASS: independent kernel variables for two live agents", flush=True)

            owned = first.tool("mathematica_list_sessions")["sessions"]
            other = second.tool("mathematica_list_sessions")["sessions"]
            assert any(s["session_id"] == wolfram_id for s in owned)
            assert all(s["session_id"] != second_id for s in owned)
            assert any(s["session_id"] == second_id for s in other)
            assert all(s["session_id"] != wolfram_id for s in other)
            print("PASS: clients see only their own sessions", flush=True)

            try:
                first.tool("mathematica_create_session")
            except RuntimeError as exc:
                assert "kernel limit" in str(exc).lower(), exc
            else:
                raise AssertionError("global two-kernel limit was not enforced")
            print("PASS: shared kernel capacity limit", flush=True)

            try:
                second.tool("mathematica_execute_code", session_id=wolfram_id, code="1+1")
            except RuntimeError as exc:
                assert "session" in str(exc).lower(), exc
            else:
                raise AssertionError("second agent executed another agent's kernel session")
            print("PASS: cross-tenant execution refused", flush=True)

            first.tool("mathematica_close_session", session_id=wolfram_id)
            wolfram_id = None
            second.tool("mathematica_close_session", session_id=second_id)
            second_id = None
            print("PASS: both agents closed their Wolfram sessions", flush=True)
        finally:
            if second_id and proc.poll() is None:
                try:
                    second.tool("mathematica_close_session", session_id=second_id)
                except Exception as exc:
                    print(f"WARN: second kernel cleanup failed: {exc}", file=sys.stderr)
            if wolfram_id and proc.poll() is None:
                try:
                    first.tool("mathematica_close_session", session_id=wolfram_id)
                except Exception as exc:
                    print(f"WARN: kernel cleanup failed: {exc}", file=sys.stderr)
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait(timeout=5)


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    executable = sys.argv[1] if len(sys.argv) > 1 else str(root / "target/debug/mathematica-mcp")
    run(executable)

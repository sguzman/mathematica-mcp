#!/usr/bin/env python3
"""Real-kernel MCP smoke test. Python standard library only.

Usage: cargo build && python3 scripts/mcp_smoke.py
Requires an installed, licensed Wolfram kernel and a usable WSTP SDK.
"""
from __future__ import annotations

import base64
import json
import os
import select
import subprocess
import sys
import time
from pathlib import Path


class MCPClient:
    def __init__(self, executable: str) -> None:
        self.proc = subprocess.Popen(
            [executable, "serve"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=None,
            bufsize=0,
        )
        self._buffer = b""
        self._next_id = 0

    def close(self) -> None:
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=5)

    def send(self, value: dict) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write((json.dumps(value, separators=(",", ":")) + "\n").encode())
        self.proc.stdin.flush()

    def read(self, timeout: float = 90) -> dict:
        deadline = time.monotonic() + timeout
        assert self.proc.stdout is not None
        while True:
            if b"\n" in self._buffer:
                line, self._buffer = self._buffer.split(b"\n", 1)
                if line:
                    return json.loads(line)
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("MCP server did not respond")
            ready, _, _ = select.select([self.proc.stdout], [], [], remaining)
            if not ready:
                raise TimeoutError("MCP server did not respond")
            chunk = os.read(self.proc.stdout.fileno(), 65536)
            if not chunk:
                raise RuntimeError(f"MCP server exited early (status {self.proc.poll()})")
            self._buffer += chunk

    def call(self, method: str, params: dict, timeout: float = 90) -> dict:
        self._next_id += 1
        request_id = self._next_id
        self.send({
            "jsonrpc": "2.0", "id": request_id, "method": method, "params": params
        })
        deadline = time.monotonic() + timeout
        while True:
            response = self.read(max(0.001, deadline - time.monotonic()))
            if response.get("id") == request_id:
                return response

    def tool(self, name: str, **arguments: object) -> dict:
        response = self.call("tools/call", {"name": name, "arguments": arguments})
        if "error" in response:
            raise RuntimeError(f"{name}: {response['error']}")
        result = response["result"]
        if result.get("isError"):
            raise RuntimeError(f"{name}: {result}")
        if isinstance(result.get("structuredContent"), dict):
            return result["structuredContent"]
        for part in result.get("content", []):
            if part.get("type") == "text":
                value = json.loads(part["text"])
                if isinstance(value, dict):
                    return value
        raise RuntimeError(f"{name}: no structured result: {result}")


def run(executable: str) -> None:
    client = MCPClient(executable)
    session_id = None
    try:
        init = client.call("initialize", {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "mathematica-mcp-smoke", "version": "1.0"},
        })
        assert "result" in init, f"initialize failed: {init}"
        client.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

        listed = client.call("tools/list", {})
        names = {tool["name"] for tool in listed["result"]["tools"]}
        required = {
            "mathematica_create_session", "mathematica_execute_code",
            "mathematica_list_sessions", "mathematica_close_session",
            "mathematica_get_finance", "mathematica_time",
        }
        assert required <= names, f"missing MCP tools: {required - names}"
        print("PASS: MCP handshake and tool enumeration")

        session_id = client.tool("mathematica_create_session")["session_id"]
        result = client.tool(
            "mathematica_execute_code", session_id=session_id, code="1+1",
            timeout_seconds=20,
        )
        assert result["output"] == "2", result
        print("PASS: real Wolfram kernel evaluation", flush=True)

        sessions = client.tool("mathematica_list_sessions")["sessions"]
        assert any(s["session_id"] == session_id for s in sessions), sessions
        print("PASS: session listing includes active session", flush=True)

        try:
            client.tool("mathematica_execute_code", session_id="invalid-id", code="1+1")
        except RuntimeError as exc:
            assert "session" in str(exc).lower(), exc
        else:
            raise AssertionError("server accepted a fabricated session ID")
        print("PASS: fabricated session ID rejected", flush=True)

        image = client.tool(
            "mathematica_execute_code",
            session_id=session_id, code="Plot[x, {x, 0, 1}]",
            timeout_seconds=45,
        )["graphics"]
        assert image and base64.b64decode(image, validate=True).startswith(
            b"\x89PNG\r\n\x1a\n"
        ), "graphics must contain a Base64-encoded PNG"
        print("PASS: graphical result encodes as PNG", flush=True)

        try:
            client.tool(
                "mathematica_execute_code", session_id=session_id,
                # CPU-bound but self-terminating if cancellation fails.
                # TimeConstrained measures kernel CPU time, not Pause wall time.
                code="Module[{start = AbsoluteTime[]}, While[AbsoluteTime[] - start < 5, Null]; 123]",
                timeout_seconds=1,
            )
        except RuntimeError as exc:
            assert "time" in str(exc).lower(), f"unexpected failure: {exc}"
        else:
            raise AssertionError("kernel TimeConstrained did not reject slow expression")
        print("PASS: CPU-bound Mathematica code times out", flush=True)

        recovery = client.tool(
            "mathematica_execute_code", session_id=session_id,
            code="2+2", timeout_seconds=20,
        )
        assert recovery["output"] == "4", recovery
        print("PASS: session recovers after a kernel-side timeout", flush=True)

        client.tool("mathematica_close_session", session_id=session_id)
        session_id = None
        print("PASS: clean shutdown", flush=True)
    finally:
        # Attempt cleanup even when an assertion aborts the test early.
        if session_id and client.proc.poll() is None:
            try:
                client.call(
                    "tools/call",
                    {
                        "name": "mathematica_close_session",
                        "arguments": {"session_id": session_id},
                    },
                    timeout=22,
                )
            except Exception as exc:
                print(f"WARN: best-effort session cleanup failed: {exc}", file=sys.stderr)
        client.close()


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    executable = sys.argv[1] if len(sys.argv) > 1 else str(root / "target/debug/mathematica-mcp")
    run(executable)

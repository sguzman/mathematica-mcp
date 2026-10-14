"""Kernel-independent tests for the MCP smoke client.

Run with: python3 -m unittest discover -s scripts -p 'test_*.py'
"""
from __future__ import annotations

import json
import os
import unittest

from mcp_smoke import MCPClient


class FakeProcess:
    def __init__(self, reader: int) -> None:
        self.stdout = os.fdopen(reader, "rb", buffering=0)

    def poll(self) -> int:
        return 0


class SmokeClientTests(unittest.TestCase):
    def client(self) -> MCPClient:
        client = MCPClient.__new__(MCPClient)
        reader, writer = os.pipe()
        self.addCleanup(os.close, writer)
        client.proc = FakeProcess(reader)
        self.addCleanup(client.proc.stdout.close)
        client._writer = writer
        client._buffer = b""
        client._next_id = 0
        return client

    def test_read_handles_fragmented_json_and_multiple_lines(self) -> None:
        client = self.client()
        os.write(client._writer, b'{"jsonrpc":"2.0","id":1,')
        os.write(client._writer, b'"result":{"ok":true}}\n{"id":2,"result":42}\n')
        self.assertTrue(client.read(1)["result"]["ok"])
        self.assertEqual(client.read(1)["result"], 42)

    def test_read_rejects_non_json_protocol_noise(self) -> None:
        client = self.client()
        os.write(client._writer, b"unexpected log on stdout\n")
        with self.assertRaises(json.JSONDecodeError):
            client.read(1)

    def test_structured_content_is_preferred(self) -> None:
        client = self.client()
        client.call = lambda *a, **k: {
            "result": {
                "structuredContent": {"output": "2"},
                "content": [{"type": "text", "text": '{"output":"wrong"}'}],
            }
        }
        self.assertEqual(client.tool("mathematica_execute_code")["output"], "2")

    def test_text_result_fallback(self) -> None:
        client = self.client()
        client.call = lambda *a, **k: {
            "result": {
                "content": [{"type": "text", "text": '{"output":"2"}'}]
            }
        }
        self.assertEqual(client.tool("mathematica_execute_code")["output"], "2")

    def test_tool_errors_rejected(self) -> None:
        client = self.client()
        client.call = lambda *a, **k: {"error": {"code": -32000, "message": "bad"}}
        with self.assertRaisesRegex(RuntimeError, "bad"):
            client.tool("mathematica_execute_code")
        client.call = lambda *a, **k: {
            "result": {"isError": True, "content": [{"type": "text", "text": "bad"}]}
        }
        with self.assertRaisesRegex(RuntimeError, "isError"):
            client.tool("mathematica_execute_code")


if __name__ == "__main__":
    unittest.main()

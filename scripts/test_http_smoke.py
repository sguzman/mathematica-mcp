"""Kernel-independent regression checks for the HTTP MCP smoke client."""
from __future__ import annotations

from io import BytesIO
import unittest

from http_smoke import read_sse_jsonrpc


class SseResponseTests(unittest.TestCase):
    def test_empty_priming_retry_and_keepalive_are_ignored(self) -> None:
        response = BytesIO(
            b": keepalive\r\n\r\n"
            b"id: 0\r\nretry: 3000\r\ndata:\r\n\r\n"
            b'data: {"jsonrpc":"2.0","method":"notifications/progress"}\r\n\r\n'
            b'data: {"jsonrpc":"2.0","id":3,"result":"other"}\r\n\r\n'
            b'data: {"jsonrpc":"2.0","id":7,"result":{"ok":true}}\r\n\r\n'
        )
        self.assertEqual(
            read_sse_jsonrpc(response, 7)["result"],
            {"ok": True},
        )

    def test_multiple_data_lines_are_joined_until_event_boundary(self) -> None:
        response = BytesIO(
            b'event: message\n'
            b'data: {"jsonrpc":"2.0",\n'
            b'data: "id":9,"result":{"answer":42}}\n\n'
        )
        self.assertEqual(read_sse_jsonrpc(response, 9)["result"]["answer"], 42)

    def test_empty_events_do_not_hide_eof(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "ended without JSON-RPC"):
            read_sse_jsonrpc(BytesIO(b"id: 0\ndata:\n\n"), 1)

    def test_nonempty_invalid_json_fails_explicitly(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "invalid JSON-RPC SSE event"):
            read_sse_jsonrpc(BytesIO(b"data: not-json\n\n"), 1)


if __name__ == "__main__":
    unittest.main()

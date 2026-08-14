from __future__ import annotations

import os
import unittest
from collections.abc import Iterator
from typing import Any
from unittest.mock import patch

import requests

import flask_app


class RawHeaders:
    """Small duplicate-preserving header collection shaped like urllib3's."""

    def __init__(self, pairs: list[tuple[str, str]]) -> None:
        self._pairs = pairs

    def __iter__(self) -> Iterator[str]:
        seen: set[str] = set()
        for key, _value in self._pairs:
            normalized = key.lower()
            if normalized not in seen:
                seen.add(normalized)
                yield key

    def get(self, key: str, default: str = "") -> str:
        values = self.getlist(key)
        return ", ".join(values) if values else default

    def getlist(self, key: str) -> list[str]:
        normalized = key.lower()
        return [value for name, value in self._pairs if name.lower() == normalized]


class StreamingBody:
    def __init__(self, chunks: list[bytes]) -> None:
        self._chunks = chunks
        self.calls: list[tuple[int, bool]] = []

    def stream(self, chunk_size: int, *, decode_content: bool) -> Iterator[bytes]:
        self.calls.append((chunk_size, decode_content))
        yield from self._chunks


class UpstreamResponse:
    def __init__(
        self,
        *,
        status_code: int = 200,
        body: list[bytes] | None = None,
        headers: list[tuple[str, str]] | None = None,
    ) -> None:
        self.status_code = status_code
        raw_headers = RawHeaders(headers or [])
        self.headers = raw_headers
        self.raw = StreamingBody(body or [])
        self.raw.headers = raw_headers
        self.close_calls = 0

    def close(self) -> None:
        self.close_calls += 1


class RecordingSession:
    def __init__(
        self,
        response: UpstreamResponse | None = None,
        error: requests.RequestException | None = None,
    ) -> None:
        self.response = response
        self.error = error
        self.requests: list[dict[str, Any]] = []
        self.close_calls = 0
        self.trust_env = True

    def request(self, **kwargs: Any) -> UpstreamResponse:
        self.requests.append(kwargs)
        if self.error is not None:
            raise self.error
        if self.response is None:
            raise AssertionError("test session has neither a response nor an error")
        return self.response

    def close(self) -> None:
        self.close_calls += 1


class LoopbackTrustBoundaryTests(unittest.TestCase):
    def test_flask_host_defaults_to_loopback_and_accepts_only_documented_hosts(self) -> None:
        with patch.dict(os.environ, {}, clear=True):
            self.assertEqual(flask_app._flask_host(), "127.0.0.1")

        for host in ("127.0.0.1", "localhost", "::1"):
            with self.subTest(host=host), patch.dict(
                os.environ, {"FLASK_HOST": host}, clear=True
            ):
                self.assertEqual(flask_app._flask_host(), host)

        for host in ("0.0.0.0", "192.0.2.10", "example.com", ""):
            with self.subTest(host=host), patch.dict(
                os.environ, {"FLASK_HOST": host}, clear=True
            ):
                with self.assertRaisesRegex(RuntimeError, "only bind to a loopback host"):
                    flask_app._flask_host()

    def test_backend_startup_requires_loopback_http_with_an_explicit_port(self) -> None:
        accepted = (
            "http://127.0.0.1:58110",
            "http://localhost:58110",
            "http://[::1]:58110",
        )
        rejected = (
            "https://127.0.0.1:58110",
            "http://127.0.0.1",
            "http://192.0.2.10:58110",
            "http://example.com:58110",
        )

        for url in accepted:
            with self.subTest(url=url):
                self.assertTrue(flask_app._is_loopback_http(url))
        for url in rejected:
            with self.subTest(url=url):
                self.assertFalse(flask_app._is_loopback_http(url))

    def test_invalid_backend_is_rejected_before_socket_or_subprocess_use(self) -> None:
        with (
            patch.object(flask_app, "_reachable") as reachable,
            patch.object(flask_app.subprocess, "Popen") as popen,
        ):
            with self.assertRaisesRegex(RuntimeError, "only supports a loopback HTTP backend"):
                flask_app.start_backend("http://192.0.2.10:58110")

        reachable.assert_not_called()
        popen.assert_not_called()

    def test_existing_port_is_reused_only_for_an_axiom_compatible_backend(self) -> None:
        with (
            patch.object(flask_app, "_reachable", return_value=True),
            patch.object(flask_app, "_compatible_backend", return_value=True),
            patch.object(flask_app.subprocess, "Popen") as popen,
        ):
            self.assertIsNone(flask_app.start_backend("http://127.0.0.1:58110"))
            popen.assert_not_called()

        with (
            patch.object(flask_app, "_reachable", return_value=True),
            patch.object(flask_app, "_compatible_backend", return_value=False),
            patch.object(flask_app.subprocess, "Popen") as popen,
        ):
            with self.assertRaisesRegex(RuntimeError, "occupied by an incompatible service"):
                flask_app.start_backend("http://127.0.0.1:58110")
            popen.assert_not_called()


class FlaskProxyTests(unittest.TestCase):
    def test_healthz_reports_backend_readiness_without_proxying(self) -> None:
        with patch.object(flask_app, "_compatible_backend", return_value=False) as probe:
            app = flask_app.create_app("http://127.0.0.1:58110/")
            response = app.test_client().get("/healthz")

        self.assertEqual(response.status_code, 200)
        self.assertEqual(
            response.get_json(),
            {
                "app": "axiom",
                "backend_ready": False,
                "backend_url": "http://127.0.0.1:58110",
                "mode": "rust-proxy",
                "status": "ok",
            },
        )
        probe.assert_called_once_with("http://127.0.0.1:58110")

    def test_proxy_forwards_request_and_streams_response_without_hop_by_hop_headers(
        self,
    ) -> None:
        upstream = UpstreamResponse(
            status_code=206,
            body=[b'{"frames":', b"[]}"],
            headers=[
                ("Content-Type", "application/json"),
                ("Set-Cookie", "first=1; Path=/"),
                ("Set-Cookie", "second=2; Path=/"),
                ("Connection", "keep-alive, X-Upstream-Internal"),
                ("X-Upstream-Internal", "do-not-forward"),
                ("Transfer-Encoding", "chunked"),
                ("X-Axiom-Result", "ok"),
            ],
        )
        session = RecordingSession(response=upstream)

        with patch.object(flask_app.requests, "Session", return_value=session):
            app = flask_app.create_app("http://127.0.0.1:58110/")
            response = app.test_client().open(
                "/api/replay?frames=7&mode=minimal",
                method="POST",
                data=b'{"seed":19}',
                content_type="application/json",
                headers={
                    "Connection": "keep-alive, X-Internal",
                    "Keep-Alive": "timeout=5",
                    "Proxy-Authorization": "local-secret",
                    "X-Internal": "do-not-forward",
                    "X-Trace": "trace-123",
                },
                buffered=True,
            )

        try:
            self.assertEqual(response.status_code, 206)
            self.assertEqual(response.get_data(), b'{"frames":[]}')
            self.assertEqual(response.headers["X-Axiom-Result"], "ok")
            self.assertEqual(
                response.headers.getlist("Set-Cookie"),
                ["first=1; Path=/", "second=2; Path=/"],
            )
            response_header_names = {name.lower() for name, _value in response.headers}
            self.assertNotIn("connection", response_header_names)
            self.assertNotIn("transfer-encoding", response_header_names)
            self.assertNotIn("x-upstream-internal", response_header_names)

            self.assertFalse(session.trust_env)
            self.assertEqual(len(session.requests), 1)
            forwarded = session.requests[0]
            self.assertEqual(forwarded["method"], "POST")
            self.assertEqual(
                forwarded["url"],
                "http://127.0.0.1:58110/api/replay?frames=7&mode=minimal",
            )
            self.assertEqual(forwarded["data"], b'{"seed":19}')
            self.assertFalse(forwarded["allow_redirects"])
            self.assertTrue(forwarded["stream"])
            self.assertEqual(forwarded["timeout"], (5, None))

            forwarded_headers = {
                name.lower(): value for name, value in forwarded["headers"].items()
            }
            self.assertEqual(forwarded_headers["content-type"], "application/json")
            self.assertEqual(forwarded_headers["x-trace"], "trace-123")
            for excluded in (
                "connection",
                "content-length",
                "host",
                "keep-alive",
                "proxy-authorization",
                "x-internal",
            ):
                self.assertNotIn(excluded, forwarded_headers)

            self.assertEqual(upstream.raw.calls, [(64 * 1024, False)])
            self.assertGreaterEqual(upstream.close_calls, 1)
            self.assertGreaterEqual(session.close_calls, 1)
        finally:
            response.close()

    def test_proxy_returns_502_and_closes_session_when_backend_fails(self) -> None:
        session = RecordingSession(error=requests.ConnectionError("connection refused"))

        with patch.object(flask_app.requests, "Session", return_value=session):
            app = flask_app.create_app("http://127.0.0.1:58110")
            response = app.test_client().get("/3d?follow=1")

        self.assertEqual(response.status_code, 502)
        self.assertEqual(
            response.get_json(),
            {
                "detail": "connection refused",
                "error": "Axiom backend is unavailable.",
            },
        )
        self.assertEqual(session.close_calls, 1)
        self.assertEqual(len(session.requests), 1)
        self.assertEqual(
            session.requests[0]["url"], "http://127.0.0.1:58110/3d?follow=1"
        )


if __name__ == "__main__":
    unittest.main()

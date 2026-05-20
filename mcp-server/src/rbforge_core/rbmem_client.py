"""HTTP client for Rust-Brain server mode."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any
from urllib.error import HTTPError
from urllib.request import Request, urlopen


class RbmemHttpError(RuntimeError):
    """Raised when the rbmem HTTP server returns an error."""


@dataclass
class RbmemHttpClient:
    base_url: str = "http://localhost:3000"

    def health(self) -> dict[str, Any]:
        return self._request("GET", "/health")

    def get_memory(self, name: str) -> dict[str, Any]:
        return self._request("GET", f"/memories/{name}")

    def put_memory(self, name: str, payload: dict[str, Any]) -> dict[str, Any]:
        return self._request("PUT", f"/memories/{name}", payload)

    def get_section(self, name: str, path: str) -> dict[str, Any]:
        return self._request("GET", f"/memories/{name}/sections/{path}")

    def put_section(self, name: str, path: str, payload: dict[str, Any]) -> dict[str, Any]:
        return self._request("PUT", f"/memories/{name}/sections/{path}", payload)

    def query(self, name: str, query: str, **options: Any) -> dict[str, Any]:
        return self._request("POST", f"/memories/{name}/query", {"query": query, **options})

    def context(self, name: str, task: str, **options: Any) -> dict[str, Any]:
        return self._request("POST", f"/memories/{name}/context", {"task": task, **options})

    def _request(
        self,
        method: str,
        path: str,
        payload: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        data = None if payload is None else json.dumps(payload).encode("utf-8")
        request = Request(
            self.base_url.rstrip("/") + path,
            data=data,
            method=method,
            headers={"content-type": "application/json"},
        )
        try:
            with urlopen(request, timeout=10) as response:  # noqa: S310 - caller controls local URL
                body = response.read().decode("utf-8")
        except HTTPError as exc:
            detail = exc.read().decode("utf-8")
            raise RbmemHttpError(f"rbmem HTTP {exc.code}: {detail}") from exc
        return json.loads(body or "{}")

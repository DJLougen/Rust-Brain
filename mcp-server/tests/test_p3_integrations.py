from __future__ import annotations

import asyncio
import base64
import json
from datetime import datetime, timedelta, timezone
from typing import Any

import pytest

from rbforge_core.marketplace import export_tool, import_tool
from rbforge_core.rbmem_client import RbmemHttpClient, RbmemHttpError
from rbforge_core.registry import audit_registry
from rbforge_mcp.server import McpHandlers


class Store:
    def __init__(self) -> None:
        self.records = {
            "old_tool": {
                "name": "old_tool",
                "description": "Old tool.",
                "schema": {"type": "object", "properties": {}, "required": []},
                "implementation": "def run():\n    return {}\n",
                "category": "analysis",
                "dependencies": [],
                "status": "validated",
                "metrics": {
                    "usage_count": 20,
                    "success_count": 2,
                    "failure_count": 18,
                    "success_rate": 0.1,
                    "last_used_at": (
                        datetime.now(timezone.utc) - timedelta(days=120)
                    ).isoformat(),
                },
            },
            "fresh_tool": {
                "name": "fresh_tool",
                "description": "Fresh tool.",
                "schema": {"type": "object", "properties": {}, "required": []},
                "implementation": "def run():\n    return {}\n",
                "category": "analysis",
                "dependencies": [],
                "status": "validated",
                "metrics": {"usage_count": 2, "success_rate": 1.0},
            },
        }
        self.writes: dict[str, Any] = {}

    def load_tool_record(self, name: str) -> dict[str, Any]:
        return self.records[name]

    def update_section(self, section: str, section_type: str, content: Any) -> None:
        self.writes[section] = content
        if section == "tools.registry":
            return
        if section.startswith("tools.custom.") and isinstance(content, dict):
            self.records[section.removeprefix("tools.custom.")] = content

    def read_registry(self) -> list[dict[str, Any]]:
        return [
            {
                "name": name,
                "section": f"tools.custom.{name}",
                "dependencies": record.get("dependencies", []),
            }
            for name, record in self.records.items()
            if record.get("status") != "archived"
        ]

    def context(
        self,
        query: str,
        *,
        resolve: bool = True,
        minified: bool = True,
        graph_depth: int = 1,
    ) -> dict[str, Any]:
        return {"context": f"context for {query}", "sections": []}


def test_registry_audit_archives_and_removes_from_active_registry() -> None:
    store = Store()

    dry_run = audit_registry(store=store, dry_run=True)
    assert dry_run == [
        {
            "tool": "old_tool",
            "action": "archive",
            "reason": "success_rate below 0.3 over at least 20 runs",
        }
    ]
    assert store.writes == {}

    applied = audit_registry(store=store, dry_run=False)

    assert applied[0]["tool"] == "old_tool"
    assert store.writes["tools.archive.old_tool"]["status"] == "archived"
    assert store.writes["tools.custom.old_tool"]["status"] == "archived"
    registry = store.writes["tools.registry"]["tools"]
    assert [item["name"] for item in registry] == ["fresh_tool"]


def test_marketplace_exports_imports_and_verifies_signature() -> None:
    pytest.importorskip("cryptography")
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
    from cryptography.hazmat.primitives.serialization import (
        Encoding,
        NoEncryption,
        PrivateFormat,
        PublicFormat,
    )

    source = Store()
    private_key = Ed25519PrivateKey.generate()
    private_bytes = private_key.private_bytes(
        Encoding.Raw,
        PrivateFormat.Raw,
        NoEncryption(),
    )
    public_bytes = private_key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)

    exported = export_tool("fresh_tool", store=source, private_key=private_bytes)
    payload = json.loads(exported)
    assert payload["signature"]["algorithm"] == "ed25519"
    assert base64.b64decode(payload["signature"]["public_key"]) == public_bytes

    target = Store()
    imported = import_tool(exported, store=target, public_key=public_bytes)
    assert imported["name"] == "fresh_tool"
    assert target.writes["tools.custom.fresh_tool"]["name"] == "fresh_tool"

    payload["tool"]["implementation"] = "tampered"
    with pytest.raises(ValueError):
        import_tool(payload, store=target, public_key=public_bytes)


def test_rbmem_http_client_shapes_json_requests(monkeypatch: pytest.MonkeyPatch) -> None:
    seen: list[tuple[str, str, dict[str, Any]]] = []

    class Response:
        def __enter__(self) -> Response:
            return self

        def __exit__(self, *_args: object) -> None:
            return None

        def read(self) -> bytes:
            return b'{"ok":true}'

    def fake_urlopen(request: Any, timeout: int) -> Response:
        seen.append(
            (
                request.get_method(),
                request.full_url,
                json.loads(request.data.decode("utf-8")) if request.data else {},
            )
        )
        assert timeout == 10
        return Response()

    monkeypatch.setattr("rbforge_core.rbmem_client.urlopen", fake_urlopen)

    client = RbmemHttpClient("http://localhost:3000/")
    assert client.query("memory", "tools", graph_depth=1) == {"ok": True}

    assert seen == [
        (
            "POST",
            "http://localhost:3000/memories/memory/query",
            {"query": "tools", "graph_depth": 1},
        )
    ]


def test_rbmem_http_client_wraps_http_errors(monkeypatch: pytest.MonkeyPatch) -> None:
    from urllib.error import HTTPError

    def fake_urlopen(_request: Any, timeout: int) -> None:
        raise HTTPError("http://server", 500, "boom", None, _ErrorBody())

    monkeypatch.setattr("rbforge_core.rbmem_client.urlopen", fake_urlopen)

    with pytest.raises(RbmemHttpError, match="rbmem HTTP 500"):
        RbmemHttpClient().health()


class _ErrorBody:
    def read(self) -> bytes:
        return b"server failed"

    def close(self) -> None:
        return None


def test_mcp_handlers_work_without_optional_mcp_package() -> None:
    store = Store()
    handlers = McpHandlers("memory.rbmem", store=store)

    resources = asyncio.run(handlers.list_resources())
    tools = asyncio.run(handlers.list_tools())
    registry = asyncio.run(handlers.read_resource("rbforge://registry"))
    query = asyncio.run(handlers.call_tool("query_memory", {"query": "fresh"}))
    update = asyncio.run(
        handlers.call_tool(
            "update_memory",
            {"section": "tools.custom.note", "type": "json", "content": {"ok": True}},
        )
    )

    assert resources[0]["uri"] == "rbforge://registry"
    assert "run_tool" in {tool["name"] for tool in tools}
    assert "fresh_tool" in registry
    assert query["context"] == "context for fresh"
    assert update == {"ok": True}
    assert store.writes["tools.custom.note"] == {"ok": True}

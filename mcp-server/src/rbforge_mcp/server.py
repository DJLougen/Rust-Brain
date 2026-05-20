"""Model Context Protocol server for RBForge."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any

from rbforge_core.forge import forge_tool
from rbforge_core.rbmem import RbmemStore
from rbforge_core.runner import run_forged_tool


@dataclass
class McpHandlers:
    memory_path: str = "memory.rbmem"
    store: Any | None = None

    def __post_init__(self) -> None:
        if self.store is None:
            self.store = RbmemStore(self.memory_path)

    async def list_resources(self) -> list[dict[str, str]]:
        return [
            {
                "uri": "rbforge://registry",
                "name": "RBForge Registry",
                "mimeType": "application/json",
            }
        ]

    async def read_resource(self, uri: str) -> str:
        if uri == "rbforge://registry":
            return json.dumps(self.store.read_registry(), indent=2, sort_keys=True)
        if uri.startswith("rbmem://"):
            _, _, path = uri.partition("/sections/")
            return self.store.context(path, resolve=True, minified=False).get("context", "")
        raise ValueError(f"unsupported resource: {uri}")

    async def list_tools(self) -> list[dict[str, Any]]:
        return [
            {
                "name": "forge_tool",
                "description": "Forge a reusable RBForge tool.",
                "inputSchema": {},
            },
            {
                "name": "run_tool",
                "description": "Run a forged RBForge tool.",
                "inputSchema": {},
            },
            {
                "name": "query_memory",
                "description": "Query RBMEM memory.",
                "inputSchema": {},
            },
            {
                "name": "update_memory",
                "description": "Update an RBMEM section.",
                "inputSchema": {},
            },
        ]

    async def call_tool(self, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        arguments = dict(arguments)
        if name == "forge_tool":
            return forge_tool(memory_path=self.memory_path, **arguments).__dict__
        if name == "run_tool":
            return run_forged_tool(
                arguments.pop("name"),
                arguments.pop("arguments"),
                memory_path=self.memory_path,
                store=self.store,
            )
        if name == "query_memory":
            return self.store.context(arguments["query"], resolve=True, minified=True)
        if name == "update_memory":
            self.store.update_section(
                arguments["section"],
                arguments.get("type", "json"),
                arguments["content"],
            )
            return {"ok": True}
        raise ValueError(f"unknown RBForge MCP tool: {name}")


def build_server(memory_path: str = "memory.rbmem") -> Any:
    try:
        from mcp.server import Server
        from mcp.types import Resource, TextContent, Tool
    except ImportError as exc:  # pragma: no cover - optional extra
        raise RuntimeError("pip install mcp to use the RBForge MCP server") from exc

    server = Server("rbforge-mcp")
    handlers = McpHandlers(memory_path)

    @server.list_resources()
    async def list_resources() -> list[Any]:
        return [
            Resource(**resource)
            for resource in await handlers.list_resources()
        ]

    @server.read_resource()
    async def read_resource(uri: str) -> str:
        return await handlers.read_resource(uri)

    @server.list_tools()
    async def list_tools() -> list[Any]:
        return [Tool(**tool) for tool in await handlers.list_tools()]

    @server.call_tool()
    async def call_tool(name: str, arguments: dict[str, Any]) -> list[Any]:
        result = await handlers.call_tool(name, arguments)
        return [TextContent(type="text", text=json.dumps(result, indent=2, sort_keys=True))]

    return server

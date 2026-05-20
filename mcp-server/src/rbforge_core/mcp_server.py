"""MCP-native server exposing RBMEM as tools.

Run as a standalone process::

    python -m rbforge_core.mcp_server

Tools exposed:
- rbmem_read: Read a section by path
- rbmem_write: Write/update a section
- rbmem_query: Query sections with pattern matching
- rbmem_health: Return health diagnostics
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

try:
    from mcp.server import Server
    from mcp.server.models import TextContent
    from mcp.types import Tool

    MCP_AVAILABLE = True
except ImportError:  # pragma: no cover - optional dependency
    MCP_AVAILABLE = False


def _get_store(memory_path: str = "MEMORY.rbmem") -> Any:
    """Lazily import and instantiate the RBMEM store."""
    from rbforge_core.rbmem import RbmemStore

    return RbmemStore(memory_path)


def _tools() -> list[Tool]:
    """Return the list of MCP tools exposed by this server."""
    return [
        Tool(
            name="rbmem_read",
            description="Read a section from an RBMEM file by path.",
            inputSchema={
                "type": "object",
                "properties": {
                    "section_path": {
                        "type": "string",
                        "description": "The RBMEM section path to read.",
                    },
                    "memory_path": {
                        "type": "string",
                        "description": "Path to the .rbmem file.",
                        "default": "MEMORY.rbmem",
                    },
                },
                "required": ["section_path"],
            },
        ),
        Tool(
            name="rbmem_write",
            description="Write or update a section in an RBMEM file.",
            inputSchema={
                "type": "object",
                "properties": {
                    "section_path": {
                        "type": "string",
                        "description": "The RBMEM section path.",
                    },
                    "content": {
                        "type": "object",
                        "description": "JSON-serialisable content to write.",
                    },
                    "memory_path": {
                        "type": "string",
                        "description": "Path to the .rbmem file.",
                        "default": "MEMORY.rbmem",
                    },
                },
                "required": ["section_path", "content"],
            },
        ),
        Tool(
            name="rbmem_query",
            description="Query RBMEM sections by pattern with graph resolution.",
            inputSchema={
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Query pattern (e.g. 'tools custom').",
                    },
                    "memory_path": {
                        "type": "string",
                        "description": "Path to the .rbmem file.",
                        "default": "MEMORY.rbmem",
                    },
                },
                "required": ["query"],
            },
        ),
        Tool(
            name="rbmem_health",
            description="Return RBMEM health diagnostics via the RBForge health scorer.",
            inputSchema={
                "type": "object",
                "properties": {
                    "memory_path": {
                        "type": "string",
                        "description": "Path to the .rbmem file.",
                        "default": "MEMORY.rbmem",
                    },
                },
                "required": [],
            },
        ),
    ]


def _text_response(data: Any) -> list[TextContent]:
    """Helper to create a TextContent response."""
    return [TextContent(type="text", text=json.dumps(data, indent=2, sort_keys=True))]


def handle_read(params: dict[str, Any]) -> list[TextContent]:
    """Handle the rbmem_read MCP tool."""
    section_path = params["section_path"]
    memory_path = params.get("memory_path", "MEMORY.rbmem")
    store = _get_store(memory_path)

    try:
        payload = store.context(section_path, resolve=True, minified=False, graph_depth=1)
        for section in payload.get("sections", []):
            if section.get("path") == section_path:
                content = section.get("content", "")
                if isinstance(content, str):
                    try:
                        content = json.loads(content)
                    except json.JSONDecodeError:
                        pass
                return _text_response(content)
        return _text_response({"error": f"section not found: {section_path}"})
    except Exception as exc:  # noqa: BLE001
        return _text_response({"error": str(exc)})


def handle_write(params: dict[str, Any]) -> list[TextContent]:
    """Handle the rbmem_write MCP tool."""
    section_path = params["section_path"]
    content = params["content"]
    memory_path = params.get("memory_path", "MEMORY.rbmem")
    store = _get_store(memory_path)

    store.update_section(section_path, "json", content, actor="mcp_server")
    return _text_response({"status": "updated", "section": section_path})


def handle_query(params: dict[str, Any]) -> list[TextContent]:
    """Handle the rbmem_query MCP tool."""
    query = params["query"]
    memory_path = params.get("memory_path", "MEMORY.rbmem")
    store = _get_store(memory_path)

    try:
        payload = store.context(query, resolve=True, minified=False, graph_depth=1)
        return _text_response(payload)
    except Exception as exc:  # noqa: BLE001
        return _text_response({"error": str(exc)})


def handle_health(params: dict[str, Any]) -> list[TextContent]:
    """Handle the rbmem_health MCP tool."""
    memory_path = params.get("memory_path", "MEMORY.rbmem")
    store = _get_store(memory_path)

    try:
        from rbforge_core.health import health_report_from_store

        report = health_report_from_store(store)
        return _text_response(
            {
                "composite_score": report.composite_score,
                "component_scores": [
                    {
                        "name": cs.name,
                        "value": cs.value,
                        "weight": cs.weight,
                        "flag": cs.flag,
                    }
                    for cs in report.component_scores
                ],
                "flags": report.flags,
                "recommendations": report.recommendations,
                "section_count": report.section_count,
                "graph_edges": report.graph_edges,
            }
        )
    except Exception as exc:  # noqa: BLE001
        return _text_response({"error": str(exc)})


def _main() -> int:
    """Entry point for the MCP server process."""
    if not MCP_AVAILABLE:
        print(
            "ERROR: mcp package is not installed. "
            "Install with: pip install 'RBForge[mcp]'",
            file=sys.stderr,
        )
        return 1

    # Determine memory path from arguments or environment
    memory_path = "MEMORY.rbmem"
    for i, arg in enumerate(sys.argv[1:]):
        if arg in {"--memory-path", "-m"} and i + 1 < len(sys.argv) - 1:
            memory_path = sys.argv[i + 2]
            break
        if arg.startswith("--memory-path="):
            memory_path = arg.split("=", 1)[1]
            break

    # Build the MCP server
    server = Server("rbforge-mcp")

    @server.list_tools()
    async def list_tools() -> list[Tool]:
        return _tools()

    @server.call_tool()
    async def call_tool(name: str, arguments: dict[str, Any]) -> list[Any]:
        handlers = {
            "rbmem_read": handle_read,
            "rbmem_write": handle_write,
            "rbmem_query": handle_query,
            "rbmem_health": handle_health,
        }
        handler = handlers.get(name)
        if handler is None:
            return [
                TextContent(type="text", text=json.dumps({"error": f"unknown tool: {name}"}))
            ]
        return handler(arguments)

    # Import stdio transport and run
    try:
        from mcp.server.stdio import stdio_server

        async with stdio_server() as (read_stream, write_stream):
            await server.run(read_stream, write_stream)
    except Exception as exc:  # noqa: BLE001
        print(f"MCP server error: {exc}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(_main())

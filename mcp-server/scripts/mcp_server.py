from __future__ import annotations

import argparse
import asyncio

from rbforge_mcp.server import build_server


def main() -> int:
    parser = argparse.ArgumentParser(prog="rbforge-mcp")
    parser.add_argument("--memory-path", default="memory.rbmem")
    parser.add_argument("--transport", choices=["stdio", "sse"], default="stdio")
    args = parser.parse_args()
    server = build_server(args.memory_path)
    if args.transport == "stdio":
        from mcp.server.stdio import stdio_server

        async def run_stdio() -> None:
            async with stdio_server() as streams:
                await server.run(streams[0], streams[1], server.create_initialization_options())

        asyncio.run(run_stdio())
        return 0
    raise SystemExit("SSE transport requires hosting with an ASGI server")


if __name__ == "__main__":
    raise SystemExit(main())

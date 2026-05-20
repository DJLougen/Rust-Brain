"""Minimal forge_tool example."""

from __future__ import annotations

from rbforge_core import forge_tool

if __name__ == "__main__":
    result = forge_tool(
        name="count_tracebacks",
        description="Count Python tracebacks in a log and return their starting line numbers.",
        schema={
            "type": "object",
            "properties": {
                "log": {"type": "string", "default": "ok\nTraceback (most recent call last):\nboom"}
            },
            "required": ["log"],
        },
        implementation=(
            "def run(log: str) -> dict:\n"
            "    lines = log.splitlines()\n"
            "    starts = [\n"
            "        idx + 1 for idx, line in enumerate(lines) if line.startswith('Traceback')\n"
            "    ]\n"
            "    return {'traceback_count': len(starts), 'starts': starts}\n"
        ),
        category="debugger",
        dependencies=["tools.builtin.ripgrep"],
        expected_output_keys=["traceback_count", "starts"],
    )
    print(result)

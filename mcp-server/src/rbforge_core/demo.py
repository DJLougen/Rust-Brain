"""Console demo entry point."""

from __future__ import annotations

from rbforge_core.forge import forge_tool


def main() -> None:
    result = forge_tool(
        name="extract_lock_waits",
        description="Extract lock wait events from a thread dump and summarize the hottest locks.",
        schema={
            "type": "object",
            "properties": {"dump": {"type": "string", "default": "Thread A waiting on lock L1"}},
            "required": ["dump"],
        },
        implementation=(
            "def run(dump: str) -> dict:\n"
            "    waits = []\n"
            "    for line in dump.splitlines():\n"
            "        if 'waiting on' in line:\n"
            "            waits.append(line.strip())\n"
            "    return {'wait_count': len(waits), 'waits': waits}\n"
        ),
        category="profiler",
        dependencies=["tools.builtin.debugger", "tools.builtin.ripgrep"],
        expected_output_keys=["wait_count", "waits"],
    )
    print(result)

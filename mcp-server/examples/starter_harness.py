"""Starter harness: search -> forge -> debug/profile -> reuse.

Run from the repo root after configuring RBMEM_CLI:

    python examples/starter_harness.py
"""

from __future__ import annotations

from pathlib import Path

from rbforge_core import forge_tool
from rbforge_core.harness import ToolHarness


def main() -> None:
    memory = Path("memory.rbmem")
    result = forge_tool(
        name="analyze_thread_contention",
        description="Summarize Python thread dumps and identify lock contention hotspots.",
        schema={
            "type": "object",
            "properties": {
                "dump": {
                    "type": "string",
                    "default": "Thread-1 waiting on lock db_pool\nThread-2 running",
                }
            },
            "required": ["dump"],
        },
        implementation=(
            "def run(dump: str) -> dict:\n"
            "    waits = []\n"
            "    locks = {}\n"
            "    for line in dump.splitlines():\n"
            "        lower = line.lower()\n"
            "        if 'waiting on lock' in lower:\n"
            "            lock = line.split('waiting on lock', 1)[1].strip().split()[0]\n"
            "            waits.append(line.strip())\n"
            "            locks[lock] = locks.get(lock, 0) + 1\n"
            "    hotspots = sorted(locks.items(), key=lambda item: item[1], reverse=True)\n"
            "    return {'wait_count': len(waits), 'hotspots': hotspots, 'waits': waits}\n"
        ),
        category="profiler",
        dependencies=["tools.builtin.debugger", "tools.builtin.ripgrep"],
        expected_output_keys=["wait_count", "hotspots", "waits"],
        memory_path=memory,
    )
    print(result.message)
    if not result.registered:
        print(result.sandbox.stderr)
        return

    harness = ToolHarness(memory)
    output = harness.call_forged(
        "analyze_thread_contention",
        {
            "dump": "\n".join(
                [
                    "Thread-A waiting on lock db_pool",
                    "Thread-B waiting on lock db_pool",
                    "Thread-C waiting on lock cache_index",
                ]
            )
        },
    )
    print(output)


if __name__ == "__main__":
    main()

"""Generate a video-style before/after RBForge demo log."""

from __future__ import annotations

from pathlib import Path


def main() -> None:
    log = Path("data/traces/demo_before_after.md")
    log.parent.mkdir(parents=True, exist_ok=True)
    log.write_text(
        """# RBForge Demo Log

## Before

Agent has `ripgrep` and `debugger_summary`, but no reusable way to rank lock
contention in thread dumps.

```text
rg "waiting on lock" logs/
debugger_summary(log_excerpt)
```

The agent can inspect raw lines, but every task repeats the same ad hoc parsing.

## Invention

```xml
<think>
I need a persistent profiler helper that converts thread dumps into lock hotspot counts.
</think>
```

```json
{"name":"forge_tool","arguments":{"name":"analyze_thread_contention","category":"profiler"}}
```

Backend:

```text
rbmem update memory.rbmem --section tools.custom.analyze_thread_contention --type json
sandbox python unittest: pass
rbmem update memory.rbmem --section tools.registry --type json
rbmem validate memory.rbmem: valid
```

## After

```json
{"name":"analyze_thread_contention","arguments":{"dump":"Thread-A waiting on lock db_pool"}}
```

```json
{"wait_count":1,"hotspots":[["db_pool",1]],"waits":["Thread-A waiting on lock db_pool"]}
```

The tool is now durable under `tools.custom.analyze_thread_contention`, indexed
by `tools.registry`, and connected to debugger/ripgrep dependencies in the RBMEM
graph.
""",
        encoding="utf-8",
    )
    print(log)


if __name__ == "__main__":
    main()

"""A/B testing helpers for forged tool variants."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any

from rbforge_core.rbmem import RbmemStore
from rbforge_core.runner import run_forged_tool

CorrectnessFn = Callable[[Any, Any], bool]


def forge_variant(
    base_tool: str,
    variant_name: str,
    new_implementation: str,
    *,
    store: Any | None = None,
    memory_path: str | Path = "memory.rbmem",
    rbmem_cli: str | None = None,
) -> dict[str, Any]:
    store = store or RbmemStore(memory_path, rbmem_cli=rbmem_cli)
    base = store.load_tool_record(base_tool)
    variant = dict(base)
    variant["name"] = variant_name
    variant["implementation"] = new_implementation
    variant["variant_of"] = base_tool
    store.update_section(f"tools.custom.{base_tool}.variants.{variant_name}", "json", variant)
    return variant


def run_ab_test(
    tool_names: list[str],
    sample_inputs: list[dict[str, Any]],
    *,
    store: Any | None = None,
    memory_path: str | Path = "memory.rbmem",
    rbmem_cli: str | None = None,
    correctness: CorrectnessFn | None = None,
    expected_outputs: list[Any] | None = None,
) -> dict[str, Any]:
    store = store or RbmemStore(memory_path, rbmem_cli=rbmem_cli)
    results: dict[str, dict[str, Any]] = {}
    expected_outputs = expected_outputs or [None] * len(sample_inputs)
    for name in tool_names:
        successes = 0
        failures = 0
        durations: list[float] = []
        outputs: list[Any] = []
        for arguments, expected in zip(sample_inputs, expected_outputs, strict=False):
            run = run_forged_tool(
                name,
                arguments,
                store=store,
                resolve_dependencies=True,
                telemetry=None,
            )
            output = run.get("result")
            ok = bool(run.get("ok"))
            if correctness is not None:
                ok = ok and correctness(output, expected)
            successes += int(ok)
            failures += int(not ok)
            durations.append(float(run.get("duration_ms", 0.0)))
            outputs.append(output)
        results[name] = {
            "successes": successes,
            "failures": failures,
            "success_rate": successes / max(1, successes + failures),
            "avg_duration_ms": sum(durations) / max(1, len(durations)),
            "outputs": outputs,
        }
    winner = max(
        results,
        key=lambda name: (results[name]["success_rate"], -results[name]["avg_duration_ms"]),
    )
    return {"schema": "rbforge.ab_test.v1", "winner": winner, "results": results}

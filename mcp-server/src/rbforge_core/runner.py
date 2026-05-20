"""Execution entry point for registered forged tools."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

from rbforge_core.dependency_resolver import DependencyResolver
from rbforge_core.models import utc_now_iso
from rbforge_core.rbmem import RbmemStore
from rbforge_core.runners import RunResult, get_runner
from rbforge_core.telemetry import TelemetrySink, emit_event


def run_forged_tool(
    name: str,
    arguments: dict[str, Any],
    *,
    memory_path: str | Path = "memory.rbmem",
    rbmem_cli: str | None = None,
    store: Any | None = None,
    resolve_dependencies: bool = True,
    telemetry: TelemetrySink | None = None,
) -> dict[str, Any]:
    store = store or RbmemStore(memory_path, rbmem_cli=rbmem_cli)
    records = _collect_records(store, [f"tools.custom.{name}"])
    order = (
        DependencyResolver(records).resolve([f"tools.custom.{name}"])
        if resolve_dependencies
        else [f"tools.custom.{name}"]
    )

    dependency_results: dict[str, Any] = {}
    final_result: RunResult | None = None
    for path in order:
        record = records[path]
        Draft202012Validator(record["schema"]).validate(arguments)
        result = get_runner(str(record.get("language", "python"))).run(
            record,
            arguments,
            dependency_results=dependency_results,
        )
        _record_run(store, path, record, result)
        emit_event(
            telemetry,
            "tool_run",
            {"tool": record["name"], "ok": result.ok, "duration_ms": result.duration_ms},
        )
        if not result.ok:
            return _response(path, result, dependency_results)
        dependency_results[path] = result.output
        final_result = result

    assert final_result is not None
    return _response(f"tools.custom.{name}", final_result, dependency_results)


def _collect_records(store: Any, requested: list[str]) -> dict[str, dict[str, Any]]:
    records: dict[str, dict[str, Any]] = {}

    def collect(path: str) -> None:
        if path in records:
            return
        name = path.rsplit(".", 1)[-1]
        record = store.load_tool_record(name)
        records[path] = record
        for dependency in record.get("dependencies", []):
            dep_path = str(dependency)
            if not dep_path.startswith("tools.custom."):
                dep_path = f"tools.custom.{dep_path}"
            collect(dep_path)

    for path in requested:
        collect(path)
    return records


def _record_run(store: Any, path: str, record: dict[str, Any], result: RunResult) -> None:
    metrics = record.setdefault(
        "metrics",
        {"usage_count": 0, "success_count": 0, "failure_count": 0, "success_rate": 0.0},
    )
    metrics["usage_count"] = int(metrics.get("usage_count", 0)) + 1
    if result.ok:
        metrics["success_count"] = int(metrics.get("success_count", 0)) + 1
    else:
        metrics["failure_count"] = int(metrics.get("failure_count", 0)) + 1
    metrics["success_rate"] = (
        int(metrics.get("success_count", 0)) / max(1, int(metrics.get("usage_count", 0)))
    )
    metrics["last_used_at"] = utc_now_iso()
    history = record.setdefault("run_history", [])
    history.append(
        {
            "ok": result.ok,
            "error": result.error,
            "duration_ms": result.duration_ms,
            "used_at": utc_now_iso(),
        }
    )
    del history[:-20]
    store.update_section(path, "json", record)


def _response(path: str, result: RunResult, dependency_results: dict[str, Any]) -> dict[str, Any]:
    return {
        "ok": result.ok,
        "name": path.rsplit(".", 1)[-1],
        "section_path": path,
        "result": result.output if result.ok else None,
        "error": result.error,
        "dependency_results": dependency_results,
        "duration_ms": result.duration_ms,
        "backend": result.backend,
    }

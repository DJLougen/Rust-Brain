from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from rbforge_core.ab_tester import run_ab_test
from rbforge_core.dependency_resolver import CircularDependencyError, DependencyResolver
from rbforge_core.improver import ToolImprover
from rbforge_core.models import ToolSpec
from rbforge_core.runner import run_forged_tool
from rbforge_core.runners import PythonRunner
from rbforge_core.sandbox import ResourceLimits, default_limits_for_category
from rbforge_core.telemetry import JsonlTelemetrySink
from rbforge_core.version import check_rbmem_compatibility


class MemoryStore:
    def __init__(self, records: dict[str, dict[str, Any]]) -> None:
        self.records = records
        self.writes: dict[str, Any] = {}

    def load_tool_record(self, name: str) -> dict[str, Any]:
        return self.records[name]

    def update_section(self, section: str, section_type: str, content: Any) -> None:
        self.writes[section] = content
        name = section.removeprefix("tools.custom.")
        if name in self.records and isinstance(content, dict):
            self.records[name] = content

    def read_registry(self) -> list[dict[str, Any]]:
        return [
            {
                "name": name,
                "section": f"tools.custom.{name}",
                "dependencies": record.get("dependencies", []),
            }
            for name, record in self.records.items()
        ]


def record(
    name: str,
    implementation: str,
    *,
    dependencies: list[str] | None = None,
    category: str = "analysis",
) -> dict[str, Any]:
    return {
        "name": name,
        "description": f"{name} test tool",
        "schema": {
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"],
        },
        "implementation": implementation,
        "category": category,
        "dependencies": dependencies or [],
        "language": "python",
        "language_config": {},
        "runtime_limits": {"cpu_sec": 2, "memory_mb": 256},
        "version": "0.1.0",
        "status": "validated",
        "metrics": {"usage_count": 0, "success_count": 0, "failure_count": 0, "success_rate": 0.0},
    }


def test_dependency_resolver_orders_dependencies_and_detects_cycles() -> None:
    records = {
        "tools.custom.clean": {"dependencies": []},
        "tools.custom.summarize": {"dependencies": ["tools.custom.clean"]},
    }

    assert DependencyResolver(records).resolve(["tools.custom.summarize"]) == [
        "tools.custom.clean",
        "tools.custom.summarize",
    ]

    records["tools.custom.clean"]["dependencies"] = ["tools.custom.summarize"]
    with pytest.raises(CircularDependencyError):
        DependencyResolver(records).resolve(["tools.custom.summarize"])


def test_python_runner_passes_dependency_context() -> None:
    result = PythonRunner().run(
        record(
            "summarize",
            "def run(text: str, rbforge_context=None) -> dict:\n"
            "    return {'seen': rbforge_context['tools.custom.clean']['cleaned']}\n",
        ),
        {"text": "Hello"},
        dependency_results={"tools.custom.clean": {"cleaned": "hello"}},
    )

    assert result.ok
    assert result.output == {"seen": "hello"}


def test_run_forged_tool_resolves_dependencies_and_updates_metrics() -> None:
    store = MemoryStore(
        {
            "clean": record(
                "clean",
                "def run(text: str) -> dict:\n    return {'cleaned': text.strip().lower()}\n",
            ),
            "summarize": record(
                "summarize",
                "def run(text: str, rbforge_context=None) -> dict:\n"
                "    return {'summary': rbforge_context['tools.custom.clean']['cleaned'][:4]}\n",
                dependencies=["tools.custom.clean"],
            ),
        }
    )

    result = run_forged_tool(
        "summarize",
        {"text": "  HELLO  "},
        store=store,
        resolve_dependencies=True,
        telemetry=None,
    )

    assert result["ok"] is True
    assert result["result"] == {"summary": "hell"}
    assert store.records["summarize"]["metrics"]["usage_count"] == 1


def test_improver_detects_failure_patterns_and_records_proposal() -> None:
    store = MemoryStore(
        {
            "extract": record(
                "extract",
                "def run(payload: dict) -> dict:\n    return {'value': payload['value']}\n",
            )
        }
    )
    store.records["extract"]["run_history"] = [
        {"ok": False, "error": "KeyError: 'value'"},
        {"ok": False, "error": "KeyError: 'value'"},
        {"ok": False, "error": "KeyError: 'value'"},
    ]

    proposal = ToolImprover(store).improve_tool("extract", auto_apply=False)

    assert proposal.should_improve
    assert "defensive access" in proposal.summary
    assert "tools.custom.extract.versions" in store.writes


def test_ab_test_compares_variants() -> None:
    store = MemoryStore(
        {
            "base": record(
                "base",
                "def run(text: str) -> dict:\n    return {'length': len(text)}\n",
            ),
            "variant": record(
                "variant",
                "def run(text: str) -> dict:\n    return {'length': len(text.strip())}\n",
            ),
        }
    )

    report = run_ab_test(
        ["base", "variant"],
        [{"text": " hi "}],
        store=store,
        correctness=lambda output, expected: output == expected,
        expected_outputs=[{"length": 2}],
    )

    assert report["winner"] == "variant"
    assert report["results"]["variant"]["successes"] == 1


def test_resource_limits_telemetry_and_version_helpers(tmp_path: Path) -> None:
    assert default_limits_for_category("debugger").cpu_sec == 10
    assert ResourceLimits(cpu_sec=1, memory_mb=64).timeout_seconds == 3

    sink = JsonlTelemetrySink(tmp_path / "events.jsonl")
    sink.emit("tool_run", {"tool": "demo", "ok": True})
    assert "tool_run" in (tmp_path / "events.jsonl").read_text(encoding="utf-8")

    assert check_rbmem_compatibility("rbmem 1.4.0").ok
    assert not check_rbmem_compatibility("rbmem 1.3.0").ok


def test_tool_spec_accepts_phase2_runtime_fields() -> None:
    spec = ToolSpec(
        name="deno_echo",
        description="Echo text through a TypeScript runtime.",
        schema={"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]},
        implementation="export function run(args) { return { text: args.text }; }",
        category="analysis",
        language="deno",
        language_config={"entry_point": "run"},
        runtime_limits={"cpu_sec": 3, "memory_mb": 128},
    )

    assert spec.language == "deno"
    assert spec.language_config["entry_point"] == "run"

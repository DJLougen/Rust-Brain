"""CPython runner for forged tools."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

from rbforge_core.runners import RunResult, ToolRunner
from rbforge_core.sandbox import ResourceLimits, default_limits_for_category


class PythonRunner(ToolRunner):
    language = "python"

    def run(
        self,
        record: dict[str, Any],
        arguments: dict[str, Any],
        *,
        dependency_results: dict[str, Any] | None = None,
    ) -> RunResult:
        limits = ResourceLimits.from_mapping(record.get("runtime_limits")) if record.get(
            "runtime_limits"
        ) else default_limits_for_category(str(record.get("category", "analysis")))
        started = time.perf_counter()
        with tempfile.TemporaryDirectory(prefix="rbforge-run-") as tmp:
            root = Path(tmp)
            (root / "tool_impl.py").write_text(str(record["implementation"]), encoding="utf-8")
            (root / "runner.py").write_text(_runner_source(str(record["name"])), encoding="utf-8")
            completed = subprocess.run(
                [
                    sys.executable,
                    str(root / "runner.py"),
                    json.dumps(arguments),
                    json.dumps(dependency_results or {}),
                ],
                cwd=root,
                text=True,
                capture_output=True,
                timeout=limits.timeout_seconds,
                check=False,
            )
        duration_ms = (time.perf_counter() - started) * 1000
        if completed.returncode != 0:
            return RunResult(
                ok=False,
                error=completed.stderr.strip() or completed.stdout.strip(),
                duration_ms=duration_ms,
                backend="python-subprocess",
            )
        try:
            output = json.loads(completed.stdout)
        except json.JSONDecodeError as exc:
            return RunResult(
                ok=False,
                error=f"tool returned non-JSON output: {exc}",
                duration_ms=duration_ms,
                backend="python-subprocess",
            )
        return RunResult(
            ok=True,
            output=output,
            duration_ms=duration_ms,
            backend="python-subprocess",
        )


def _runner_source(tool_name: str) -> str:
    return f"""import importlib
import inspect
import json
import sys

args = json.loads(sys.argv[1])
context = json.loads(sys.argv[2])
module = importlib.import_module("tool_impl")
func = getattr(module, {tool_name!r}, None) or getattr(module, "run")
signature = inspect.signature(func)
if "rbforge_context" in signature.parameters:
    args["rbforge_context"] = context
result = func(**args)
print(json.dumps(result))
"""

"""Deno/TypeScript runner for forged tools."""

from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any

from rbforge_core.runners import RunResult, ToolRunner
from rbforge_core.sandbox import ResourceLimits, default_limits_for_category


class DenoRunner(ToolRunner):
    language = "deno"

    def run(
        self,
        record: dict[str, Any],
        arguments: dict[str, Any],
        *,
        dependency_results: dict[str, Any] | None = None,
    ) -> RunResult:
        if not shutil.which("deno"):
            return RunResult(ok=False, error="deno executable not found", backend="deno")
        limits = ResourceLimits.from_mapping(record.get("runtime_limits")) if record.get(
            "runtime_limits"
        ) else default_limits_for_category(str(record.get("category", "analysis")))
        entry_point = str(record.get("language_config", {}).get("entry_point", "run"))
        started = time.perf_counter()
        with tempfile.TemporaryDirectory(prefix="rbforge-deno-") as tmp:
            root = Path(tmp)
            (root / "tool.ts").write_text(str(record["implementation"]), encoding="utf-8")
            (root / "runner.ts").write_text(_runner_source(entry_point), encoding="utf-8")
            completed = subprocess.run(
                [
                    "deno",
                    "run",
                    "--allow-none",
                    str(root / "runner.ts"),
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
                backend="deno",
            )
        return RunResult(
            ok=True,
            output=json.loads(completed.stdout),
            duration_ms=duration_ms,
            backend="deno",
        )


def _runner_source(entry_point: str) -> str:
    return f"""import * as tool from "./tool.ts";
const args = JSON.parse(Deno.args[0]);
const context = JSON.parse(Deno.args[1]);
const fn = tool[{entry_point!r}];
const result = await fn(args, context);
console.log(JSON.stringify(result));
"""

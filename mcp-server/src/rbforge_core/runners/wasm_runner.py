"""Wasmtime runner for base64-encoded WASM forged tools."""

from __future__ import annotations

import base64
import time
from typing import Any

from rbforge_core.runners import RunResult, ToolRunner


class WasmRunner(ToolRunner):
    language = "wasm"

    def run(
        self,
        record: dict[str, Any],
        arguments: dict[str, Any],
        *,
        dependency_results: dict[str, Any] | None = None,
    ) -> RunResult:
        started = time.perf_counter()
        try:
            import wasmtime  # type: ignore[import-not-found]
        except ImportError:
            return RunResult(ok=False, error="wasmtime is not installed", backend="wasmtime")

        try:
            module_bytes = base64.b64decode(str(record["implementation"]))
            engine = wasmtime.Engine()
            module = wasmtime.Module(engine, module_bytes)
            store = wasmtime.Store(engine)
            instance = wasmtime.Instance(store, module, [])
            entry_point = str(record.get("language_config", {}).get("entry_point", "run"))
            func = instance.exports(store)[entry_point]
            raw_result = func(store)
        except Exception as exc:  # noqa: BLE001 - returned as clean tool output
            return RunResult(
                ok=False,
                error=str(exc),
                duration_ms=(time.perf_counter() - started) * 1000,
                backend="wasmtime",
            )
        return RunResult(
            ok=True,
            output={
                "result": raw_result,
                "arguments": arguments,
                "context": dependency_results or {},
            },
            duration_ms=(time.perf_counter() - started) * 1000,
            backend="wasmtime",
        )

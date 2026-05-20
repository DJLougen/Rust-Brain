"""Runtime adapters for forged tools."""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class RunResult:
    ok: bool
    output: Any = None
    error: str = ""
    duration_ms: float = 0.0
    backend: str = ""


class ToolRunner(ABC):
    language: str

    @abstractmethod
    def run(
        self,
        record: dict[str, Any],
        arguments: dict[str, Any],
        *,
        dependency_results: dict[str, Any] | None = None,
    ) -> RunResult:
        """Execute one forged tool record."""


def get_runner(language: str) -> ToolRunner:
    normalized = language.lower()
    if normalized == "python":
        from rbforge_core.runners.python_runner import PythonRunner

        return PythonRunner()
    if normalized == "wasm":
        from rbforge_core.runners.wasm_runner import WasmRunner

        return WasmRunner()
    if normalized in {"deno", "typescript"}:
        from rbforge_core.runners.deno_runner import DenoRunner

        return DenoRunner()
    raise ValueError(f"unsupported forged tool language: {language}")


def __getattr__(name: str) -> object:
    if name == "PythonRunner":
        from rbforge_core.runners.python_runner import PythonRunner

        return PythonRunner
    raise AttributeError(name)


__all__ = ["PythonRunner", "RunResult", "ToolRunner", "get_runner"]

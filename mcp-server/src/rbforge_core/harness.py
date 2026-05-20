"""Starter harness combining built-ins with forged runtime tools."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

from rbforge_core.ab_tester import run_ab_test
from rbforge_core.debugger import debugger_signal_report
from rbforge_core.improver import ToolImprover
from rbforge_core.rbmem import RbmemStore
from rbforge_core.registry import audit_registry
from rbforge_core.runner import run_forged_tool


class ToolHarness:
    def __init__(
        self,
        memory_path: str | Path = "memory.rbmem",
        rbmem_cli: str | None = None,
        store: Any | None = None,
    ) -> None:
        self.memory_path = memory_path
        self.rbmem_cli = rbmem_cli
        self.store = store or RbmemStore(memory_path, rbmem_cli=rbmem_cli)

    def ripgrep(self, pattern: str, root: str | Path = ".") -> str:
        cmd = ["rg", "-n", pattern, str(root)]
        completed = subprocess.run(cmd, text=True, capture_output=True, check=False)
        if completed.returncode not in {0, 1}:
            raise RuntimeError(completed.stderr.strip())
        return completed.stdout

    def debugger_summary(self, text: str) -> dict[str, Any]:
        return debugger_signal_report(text)

    def call_forged(self, name: str, arguments: dict[str, Any]) -> Any:
        result = run_forged_tool(
            name,
            arguments,
            memory_path=self.memory_path,
            rbmem_cli=self.rbmem_cli,
            store=self.store,
            resolve_dependencies=True,
            telemetry=None,
        )
        if not result["ok"]:
            raise RuntimeError(result["error"])
        return result["result"]

    def improve_forged(self, name: str, *, auto_apply: bool = False) -> Any:
        return ToolImprover(self.store).improve_tool(name, auto_apply=auto_apply)

    def ab_test_forged(
        self,
        tool_names: list[str],
        sample_inputs: list[dict[str, Any]],
    ) -> dict[str, Any]:
        return run_ab_test(tool_names, sample_inputs, store=self.store)

    def audit_forged_registry(self, *, dry_run: bool = True) -> list[dict[str, Any]]:
        return audit_registry(store=self.store, dry_run=dry_run)

    def _load_tool_record(self, name: str) -> dict[str, Any]:
        return self.store.load_tool_record(name)

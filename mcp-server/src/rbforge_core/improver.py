"""Tool auto-improvement proposal engine."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from rbforge_core.models import utc_now_iso
from rbforge_core.rbmem import RbmemStore


@dataclass(frozen=True)
class ImprovementProposal:
    tool: str
    should_improve: bool
    summary: str
    proposed_implementation: str
    patterns: list[str]


class ToolImprover:
    def __init__(self, store: Any) -> None:
        self.store = store

    def improve_tool(self, name: str, *, auto_apply: bool = False) -> ImprovementProposal:
        record = self.store.load_tool_record(name)
        proposal = self.propose(name, record)
        versions = list(record.get("versions", []))
        versions.append(
            {
                "version": record.get("version", "0.1.0"),
                "implementation": record.get("implementation", ""),
                "archived_at": utc_now_iso(),
                "reason": proposal.summary,
            }
        )
        self.store.update_section(f"tools.custom.{name}.versions", "json", versions)
        self.store.update_section(
            f"tools.custom.{name}.improvement_proposals.latest",
            "json",
            proposal.__dict__,
        )
        if auto_apply and proposal.should_improve:
            record["implementation"] = proposal.proposed_implementation
            record["improved_at"] = utc_now_iso()
            self.store.update_section(f"tools.custom.{name}", "json", record)
        return proposal

    def propose(self, name: str, record: dict[str, Any]) -> ImprovementProposal:
        runs = list(record.get("run_history", []))[-20:]
        failures = [run for run in runs[-10:] if not run.get("ok")]
        metrics = record.get("metrics", {})
        success_rate = float(metrics.get("success_rate", 1.0))
        should_improve = len(failures) >= 3 or (len(runs) >= 20 and success_rate < 0.7)
        patterns = _patterns_from_errors([str(run.get("error", "")) for run in failures])
        summary = _summary(patterns) if should_improve else "no improvement trigger reached"
        implementation = str(record.get("implementation", ""))
        proposed = (
            _annotate_implementation(implementation, patterns)
            if should_improve
            else implementation
        )
        return ImprovementProposal(
            tool=name,
            should_improve=should_improve,
            summary=summary,
            proposed_implementation=proposed,
            patterns=patterns,
        )


def improve_tool(
    name: str,
    memory_path: str | Path = "memory.rbmem",
    *,
    auto_apply: bool = False,
    rbmem_cli: str | None = None,
) -> ImprovementProposal:
    return ToolImprover(RbmemStore(memory_path, rbmem_cli=rbmem_cli)).improve_tool(
        name,
        auto_apply=auto_apply,
    )


def _patterns_from_errors(errors: list[str]) -> list[str]:
    patterns: list[str] = []
    joined = "\n".join(errors)
    if "KeyError" in joined or "IndexError" in joined:
        patterns.append("defensive access")
    if "TypeError" in joined:
        patterns.append("type validation")
    if "ValueError" in joined:
        patterns.append("input sanitization")
    return patterns or ["general error handling"]


def _summary(patterns: list[str]) -> str:
    return "propose " + ", ".join(patterns)


def _annotate_implementation(implementation: str, patterns: list[str]) -> str:
    guidance = "\n".join(f"# RBForge improvement hint: add {pattern}." for pattern in patterns)
    return f"{guidance}\n{implementation}"

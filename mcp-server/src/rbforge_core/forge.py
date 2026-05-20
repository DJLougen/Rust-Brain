"""Meta-tool entry point used by Hermes agents."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from rbforge_core.models import ForgeResult, ToolSpec
from rbforge_core.rbmem import RbmemStore
from rbforge_core.sandbox import SandboxExecutor
from rbforge_core.telemetry import JsonlTelemetrySink, emit_event
from rbforge_core.trajectory import TrajectoryLogger
from rbforge_core.validation import validate_tool_spec


def forge_tool(
    *,
    name: str,
    description: str,
    schema: dict[str, Any],
    implementation: str,
    category: str,
    dependencies: list[str] | None = None,
    memory_path: str | Path = "memory.rbmem",
    language: str = "python",
    language_config: dict[str, Any] | None = None,
    runtime_limits: dict[str, Any] | None = None,
    expected_args: dict[str, Any] | None = None,
    expected_output_keys: list[str] | None = None,
    high_impact: bool = False,
    rbmem_cli: str | None = None,
    trace_path: str | Path | None = "data/traces/RBForge.jsonl",
) -> ForgeResult:
    """Forge, validate, persist, and register a runtime tool.

    This function is intentionally shaped like a Hermes-callable meta-tool. The
    caller supplies the complete implementation; RBForge owns validation,
    timestamp-safe persistence, registration, and trace logging.
    """
    spec = ToolSpec(
        name=name,
        description=description,
        schema=schema,
        implementation=implementation,
        category=category,
        dependencies=dependencies or [],
        language=language,  # type: ignore[arg-type]
        language_config=language_config or {},
        runtime_limits=runtime_limits or {},
        expected_args=expected_args,
        expected_output_keys=expected_output_keys or [],
        high_impact=high_impact,
    )
    validate_tool_spec(spec)

    store = RbmemStore(memory_path, rbmem_cli=rbmem_cli)
    logger = TrajectoryLogger(trace_path) if trace_path else None
    if logger:
        logger.record("forge_requested", {"tool": name, "category": category})
    telemetry = JsonlTelemetrySink(trace_path) if trace_path else None
    emit_event(telemetry, "tool_forged", {"tool": name, "category": category, "phase": "requested"})

    store.persist_candidate(spec)
    if logger:
        logger.record("candidate_persisted", {"section": spec.section_path})

    sandbox = SandboxExecutor().validate(spec)
    if logger:
        logger.record(
            "sandbox_finished",
            {
                "tool": name,
                "ok": sandbox.ok,
                "backend": sandbox.backend,
                "returncode": sandbox.returncode,
                "static_warnings": sandbox.static_warnings,
            },
        )

    review_required = high_impact or category in {"memory", "web_bubble", "filesystem", "shell"}
    registered = False
    registry_size = len(store.read_registry())
    if sandbox.ok and not review_required:
        registry_size = store.register_validated_tool(
            spec,
            {
                "backend": sandbox.backend,
                "returncode": sandbox.returncode,
                "stdout_tail": sandbox.stdout[-1200:],
                "stderr_tail": sandbox.stderr[-1200:],
            },
        )
        registered = True
        if logger:
            logger.record("tool_registered", {"tool": name, "registry_size": registry_size})
        emit_event(
            telemetry,
            "tool_forged",
            {"tool": name, "category": category, "phase": "registered"},
        )
    elif review_required and sandbox.ok:
        store.update_section(
            f"tools.review_queue.{name}",
            "json",
            {
                "tool": spec.section_path,
                "reason": "high impact category requires human review",
                "category": category,
            },
        )
        if logger:
            logger.record("review_queued", {"tool": name, "category": category})

    return ForgeResult(
        ok=sandbox.ok,
        name=name,
        section_path=spec.section_path,
        registered=registered,
        rbmem_path=str(Path(memory_path)),
        sandbox=sandbox,
        registry_size=registry_size,
        review_required=review_required,
        message=_message(sandbox.ok, registered, review_required),
        rbmem_diagnostics=store.doctor(),
        rbmem_context_preview=store.context_preview(spec.name),
    )


def _message(ok: bool, registered: bool, review_required: bool) -> str:
    if not ok:
        return "tool candidate persisted but sandbox validation failed"
    if review_required:
        return "tool candidate passed validation and is queued for human review"
    if registered:
        return "tool candidate passed validation and was registered"
    return "tool candidate processed"

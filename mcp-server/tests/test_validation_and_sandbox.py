from __future__ import annotations

from rbforge_core.models import ToolSpec
from rbforge_core.sandbox import SandboxExecutor
from rbforge_core.validation import ToolSpecError, validate_tool_spec


def test_valid_python_tool_passes_local_sandbox() -> None:
    spec = ToolSpec(
        name="count_items",
        description="Count items in a list and return the total.",
        schema={
            "type": "object",
            "properties": {"items": {"type": "array", "default": [1, 2, 3]}},
            "required": ["items"],
        },
        implementation="def run(items: list) -> dict:\n    return {'count': len(items)}\n",
        category="debugger",
        expected_output_keys=["count"],
    )

    validate_tool_spec(spec)
    result = SandboxExecutor(prefer_docker=False).validate(spec)

    assert result.ok
    assert result.backend == "local-subprocess"


def test_validation_rejects_bad_name() -> None:
    spec = ToolSpec(
        name="Bad-Name",
        description="Count items in a list and return the total.",
        schema={"type": "object", "properties": {}, "required": []},
        implementation="def run() -> dict:\n    return {}\n",
        category="debugger",
    )

    try:
        validate_tool_spec(spec)
    except ToolSpecError as exc:
        assert "snake_case" in str(exc)
    else:
        raise AssertionError("expected ToolSpecError")

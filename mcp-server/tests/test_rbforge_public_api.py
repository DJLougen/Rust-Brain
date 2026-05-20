from __future__ import annotations

from pathlib import Path

from rbforge_core.rbmem import RbmemStore, patch_section_graph
from rbforge_core.runner import run_forged_tool
from rbforge_core.validation import validate_spec, ToolSpecError, sample_args
from rbforge_core.models import ToolSpec


def test_public_validate_spec_uses_jsonschema() -> None:
    spec = ToolSpec(
        name="rank_locks",
        description="Rank lock contention hotspots from a thread dump.",
        schema={
            "type": "object",
            "properties": {"dump": {"type": "string", "default": "Thread waiting on lock db"}},
            "required": ["dump"],
        },
        implementation="def run(dump: str) -> dict:\n    return {'wait_count': 0}\n",
        category="profiler",
        expected_output_keys=["wait_count"],
    )

    validate_spec(spec)
    assert sample_args(spec.schema)["dump"].startswith("Thread")


def test_patch_section_graph_replaces_duplicate_graph_blocks() -> None:
    text = """rbmem# RBMEM v1.3

[SECTION: tools.custom.demo]
type: json
graph:
  node_type: "tool"
  relations:
    - to: "old"
      type: "depends_on"
temporal:
  created_at: "2026-04-28T00:00:00Z"
  updated_at: "2026-04-28T00:00:00Z"
  expires_at: null
graph:
  node_type: "tool"
  relations:
    - to: "duplicate"
      type: "depends_on"
content: |
  {"name":"demo"}
[END SECTION]
"""

    patched = patch_section_graph(
        text,
        "tools.custom.demo",
        "tool",
        [{"to": "tools.registry", "type": "registered_in"}],
    )

    assert patched.count("graph:") == 1
    assert 'to: "tools.registry"' in patched
    assert 'updated_at: "2026-04-28T00:00:00Z"' in patched


def test_run_forged_tool_updates_metrics(tmp_path: Path) -> None:
    class FakeStore:
        record = {
            "name": "count_words",
            "description": "Count words in text.",
            "schema": {
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"],
            },
            "implementation": (
                "def run(text: str) -> dict:\n"
                "    return {'word_count': len(text.split())}\n"
            ),
            "language": "python",
            "category": "debugger",
            "dependencies": [],
            "version": "0.1.0",
            "status": "validated",
            "registered_at": "2026-04-28T00:00:00Z",
            "metrics": {
                "usage_count": 0,
                "success_count": 0,
                "failure_count": 0,
                "success_rate": 0.0,
                "last_used_at": None,
            },
        }

        def __init__(self, memory_path: str | Path, rbmem_cli: str | None = None) -> None:
            self.memory_path = memory_path
            self.rbmem_cli = rbmem_cli

        def load_tool_record(self, name: str) -> dict[str, object]:
            return self.record

        def update_section(self, section: str, section_type: str, content: dict[str, object], **kwargs: object) -> None:
            self.record = content

    result = run_forged_tool(
        name="count_words",
        arguments={"text": "one two"},
        memory_path=tmp_path / "memory.rbmem",
        store=FakeStore(tmp_path / "memory.rbmem"),
        resolve_dependencies=False,
    )

    assert result["ok"] is True
    assert result["result"] == {"word_count": 2}


def test_web_bubble_tools_can_import_http_clients() -> None:
    spec = ToolSpec(
        name="fetch_social_json",
        description="Fetch JSON from a social monitoring endpoint.",
        schema={
            "type": "object",
            "properties": {"url": {"type": "string", "default": "https://example.com"}},
            "required": ["url"],
        },
        implementation=(
            "import urllib.request\n\n"
            "def run(url: str) -> dict:\n"
            "    return {'url': url, 'client': urllib.request.__name__}\n"
        ),
        category="web_bubble",
        expected_output_keys=["url", "client"],
    )

    validate_spec(spec)


def test_non_web_tools_still_reject_http_clients() -> None:
    spec = ToolSpec(
        name="fetch_social_json",
        description="Fetch JSON from a social monitoring endpoint.",
        schema={
            "type": "object",
            "properties": {"url": {"type": "string", "default": "https://example.com"}},
            "required": ["url"],
        },
        implementation=(
            "import urllib.request\n\n"
            "def run(url: str) -> dict:\n"
            "    return {'url': url}\n"
        ),
        category="debugger",
    )
    try:
        validate_spec(spec)
    except ToolSpecError as exc:
        assert "forbidden import" in str(exc)
    else:
        raise AssertionError("expected ToolSpecError")


def test_shell_tools_can_import_subprocess_but_other_tools_cannot() -> None:
    shell_spec = ToolSpec(
        name="echo_command",
        description="Run a constrained shell echo command.",
        schema={"type": "object", "properties": {}, "required": []},
        implementation=(
            "import subprocess\n\n"
            "def run() -> dict:\n"
            "    return {'module': subprocess.__name__}\n"
        ),
        category="shell",
    )
    validate_spec(shell_spec)

    debugger_spec = ToolSpec(
        name="echo_command",
        description="Run a constrained shell echo command.",
        schema={"type": "object", "properties": {}, "required": []},
        implementation=shell_spec.implementation,
        category="debugger",
    )
    try:
        validate_spec(debugger_spec)
    except ToolSpecError as exc:
        assert "forbidden import" in str(exc)
    else:
        raise AssertionError("expected ToolSpecError")

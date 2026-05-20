from __future__ import annotations

from typing import Any

from rbforge_core.harness import ToolHarness


class HarnessStore:
    def __init__(self) -> None:
        self.records = {
            "clean": {
                "name": "clean",
                "description": "Clean text.",
                "schema": {
                    "type": "object",
                    "properties": {"text": {"type": "string"}},
                    "required": ["text"],
                },
                "implementation": (
                    "def run(text: str) -> dict:\n"
                    "    return {'cleaned': text.strip()}\n"
                ),
                "category": "analysis",
                "dependencies": [],
                "language": "python",
                "runtime_limits": {"cpu_sec": 2, "memory_mb": 128},
                "metrics": {"usage_count": 0, "success_count": 0, "failure_count": 0},
            },
            "summarize": {
                "name": "summarize",
                "description": "Summarize clean text.",
                "schema": {
                    "type": "object",
                    "properties": {"text": {"type": "string"}},
                    "required": ["text"],
                },
                "implementation": (
                    "def run(text: str, rbforge_context=None) -> dict:\n"
                    "    return {'summary': rbforge_context['tools.custom.clean']['cleaned']}\n"
                ),
                "category": "analysis",
                "dependencies": ["tools.custom.clean"],
                "language": "python",
                "runtime_limits": {"cpu_sec": 2, "memory_mb": 128},
                "metrics": {"usage_count": 0, "success_count": 0, "failure_count": 0},
            },
        }
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
                "dependencies": record["dependencies"],
            }
            for name, record in self.records.items()
        ]


def test_harness_calls_forged_tools_with_dependencies() -> None:
    harness = ToolHarness(store=HarnessStore())

    result = harness.call_forged("summarize", {"text": "  hello  "})

    assert result == {"summary": "hello"}


def test_harness_exposes_improvement_proposals() -> None:
    store = HarnessStore()
    store.records["clean"]["run_history"] = [
        {"ok": False, "error": "TypeError: expected str"},
        {"ok": False, "error": "TypeError: expected str"},
        {"ok": False, "error": "TypeError: expected str"},
    ]
    harness = ToolHarness(store=store)

    proposal = harness.improve_forged("clean")

    assert proposal.should_improve
    assert "type validation" in proposal.summary

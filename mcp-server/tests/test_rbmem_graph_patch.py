from __future__ import annotations

import json
import subprocess
from pathlib import Path

from rbforge_core.rbmem import RbmemStore


class NoopRbmemStore(RbmemStore):
    def __init__(self, memory_path: Path) -> None:
        self.memory_path = memory_path
        self.rbmem_cli = "rbmem"

    def validate(self) -> None:
        return None


def test_apply_graph_inserts_relations_without_touching_temporal(tmp_path: Path) -> None:
    memory = tmp_path / "memory.rbmem"
    memory.write_text(
        """rbmem# RBMEM v1.3 - Rust-Brain Memory Format

meta:
  version: 1.3

[SECTION: tools.custom.demo]
type: json
temporal:
  created_at: "2026-04-28T00:00:00Z"
  updated_at: "2026-04-28T00:00:00Z"
  expires_at: null
content: |
  {"name":"demo"}
[END SECTION]
""",
        encoding="utf-8",
    )

    store = NoopRbmemStore(memory)
    store.apply_graph(
        "tools.custom.demo",
        node_type="tool",
        relations=[{"to": "tools.registry", "type": "registered_in"}],
    )

    text = memory.read_text(encoding="utf-8")
    assert 'node_type: "tool"' in text
    assert 'to: "tools.registry"' in text
    assert 'updated_at: "2026-04-28T00:00:00Z"' in text


class JsonRbmemStore(RbmemStore):
    def __init__(self, memory_path: Path) -> None:
        self.memory_path = memory_path
        self.rbmem_cli = "rbmem"
        self.commands: list[list[str]] = []

    def ensure(self) -> None:
        return None

    def _run(
        self,
        args: list[str],
        *,
        capture: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        self.commands.append(args)
        if args == ["rbmem", "--version"]:
            return subprocess.CompletedProcess(args, 0, stdout="rbmem 0.4.0\n", stderr="")
        if args[1:3] == ["hermes", "doctor"]:
            return subprocess.CompletedProcess(
                args,
                0,
                stdout=json.dumps(
                    {
                        "schema": "rbmem.hermes.doctor.v1",
                        "hermes_load": {"status": "ok"},
                    }
                ),
                stderr="",
            )
        if args[1] == "query":
            return subprocess.CompletedProcess(
                args,
                0,
                stdout=json.dumps(
                    {
                        "schema": "rbmem.context.v1",
                        "context": "[tools.registry] json {}",
                        "sections": [
                            {
                                "path": "tools.registry",
                                "content": json.dumps({"tools": [{"name": "demo"}]}),
                            }
                        ],
                    }
                ),
                stderr="",
            )
        raise AssertionError(f"unexpected command: {args}")


def test_store_uses_rbmem_v04_json_doctor_and_context(tmp_path: Path) -> None:
    store = JsonRbmemStore(tmp_path / "memory.rbmem")

    assert store.rbmem_version() == "rbmem 0.4.0"
    assert store.doctor()["schema"] == "rbmem.hermes.doctor.v1"
    assert store.context("tools registry")["schema"] == "rbmem.context.v1"
    assert store.read_registry() == [{"name": "demo"}]

    assert any("--format" in command and "json" in command for command in store.commands)

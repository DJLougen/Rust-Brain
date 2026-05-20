from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from rbforge_core.doctor import build_report, format_text_report, main, run_doctor


class DoctorStore:
    def __init__(self, memory_path: Path) -> None:
        self.memory_path = memory_path
        self.rbmem_cli = "rbmem"

    def rbmem_version(self) -> str:
        return "rbmem 1.4.0"

    def doctor(self) -> dict[str, Any]:
        self.memory_path.write_text("memory", encoding="utf-8")
        return {
            "schema": "rbmem.hermes.doctor.v1",
            "hermes_load": {"status": "ok"},
        }

    def read_registry(self) -> list[dict[str, Any]]:
        return [
            {
                "name": "demo",
                "section": "tools.custom.demo",
                "status": "validated",
                "category": "debugger",
                "dependencies": [],
                "success_rate": 0.75,
            }
        ]

    def context(
        self,
        query: str,
        *,
        resolve: bool = True,
        minified: bool = False,
        graph_depth: int = 1,
    ) -> dict[str, Any]:
        assert query == "tools custom"
        return {
            "schema": "rbmem.context.v1",
            "sections": [
                {
                    "path": "tools.custom.demo",
                    "content": json.dumps(
                        {
                            "name": "demo",
                            "status": "validated",
                            "category": "debugger",
                            "dependencies": [],
                            "metrics": {"success_rate": 0.75},
                        }
                    ),
                }
            ],
        }


def test_doctor_report_collects_versions_health_and_metrics(tmp_path: Path) -> None:
    report = build_report(DoctorStore(tmp_path / "memory.rbmem"), rbforge_version="0.test")

    assert report["schema"] == "rbforge.doctor.v1"
    assert report["rbforge_version"] == "0.test"
    assert report["rbmem_cli_version"] == "rbmem 1.4.0"
    assert report["rbmem_compatibility"]["ok"] is True
    assert report["memory_file"]["health"] == "ok"
    assert report["registry_size"] == 1
    assert report["forged_tools"] == 1
    assert report["validated_tools"] == 1
    assert report["validation_rate"] == 1.0
    assert report["average_success_rate"] == 0.75
    assert report["debugger_tools"] == 1
    assert report["debugger_validation_rate"] == 1.0
    assert report["debugger_average_success_rate"] == 0.75
    assert report["category_metrics"]["debugger"]["tools"] == 1


def test_text_report_formats_easy_to_read_metrics(tmp_path: Path) -> None:
    report = build_report(DoctorStore(tmp_path / "memory.rbmem"), rbforge_version="0.test")
    text = format_text_report(report)

    assert "RBForge doctor" in text
    assert "rbforge-version: 0.test" in text
    assert "memory-health: ok" in text
    assert "validation-rate: 100.0%" in text
    assert "average-success-rate: 75.0%" in text
    assert "debugger-tools: 1" in text


def test_run_doctor_uses_real_store_arguments(monkeypatch: Any, tmp_path: Path) -> None:
    created: list[tuple[str, str | None]] = []

    class FakeStore(DoctorStore):
        def __init__(self, memory_path: str, rbmem_cli: str | None = None) -> None:
            created.append((memory_path, rbmem_cli))
            super().__init__(tmp_path / "memory.rbmem")

    monkeypatch.setattr("rbforge_core.doctor.RbmemStore", FakeStore)
    args = argparse.Namespace(
        memory_path="custom.rbmem",
        rbmem_cli="custom-rbmem",
        format="json",
    )

    report = run_doctor(args)

    assert created == [("custom.rbmem", "custom-rbmem")]
    assert report["registry_size"] == 1


def test_doctor_main_prints_json(monkeypatch: Any, capsys: Any, tmp_path: Path) -> None:
    class FakeStore(DoctorStore):
        def __init__(self, memory_path: str, rbmem_cli: str | None = None) -> None:
            super().__init__(tmp_path / "memory.rbmem")

    monkeypatch.setattr("rbforge_core.doctor.RbmemStore", FakeStore)

    assert main(["custom.rbmem", "--format", "json"]) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["schema"] == "rbforge.doctor.v1"

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from rbforge_core.eval import (
    DEFAULT_DEBUGGER_CASES,
    build_debugger_eval,
    format_debugger_eval,
    load_debugger_cases,
    main,
)


def test_debugger_eval_compares_debugger_first_to_baseline() -> None:
    report = build_debugger_eval(
        [
            {
                "id": "case-one",
                "log": (
                    "FAILED tests/test_cache.py::test_lock_timeout\n"
                    "Traceback (most recent call last):\n"
                    '  File "app/cache.py", line 42, in get\n'
                    "TimeoutError: lock wait exceeded"
                ),
                "expected": {
                    "exception": "TimeoutError",
                    "file": "app/cache.py",
                    "test": "tests/test_cache.py::test_lock_timeout",
                },
                "baseline": {"turns": 6, "root_cause_hit": False},
                "debugger": {"used": True, "turns": 3, "reusable_debugger_created": True},
            }
        ]
    )

    assert report["schema"] == "rbforge.eval.debugger.v1"
    assert report["cases"] == 1
    assert report["families"] == 1
    assert report["debugger_use_rate"] == 1.0
    assert report["root_cause_hit_rate"] == 1.0
    assert report["baseline_root_cause_hit_rate"] == 0.0
    assert report["avg_turn_reduction"] == 0.5
    assert report["estimated_turns_saved"] == 3
    assert report["reusable_debuggers_created"] == 1
    assert report["family_metrics"]["debugging"]["turns_saved"] == 3


def test_debugger_eval_text_is_tweetable() -> None:
    report = build_debugger_eval(
        [
            {
                "id": "case-one",
                "log": 'FAILED t.py::test_a\nFile "a.py", line 1\nValueError: bad',
                "expected": {"exception": "ValueError", "file": "a.py", "test": "t.py::test_a"},
                "baseline": {"turns": 4, "root_cause_hit": False},
                "debugger": {"used": True, "turns": 2, "reusable_debugger_created": False},
            }
        ]
    )

    text = format_debugger_eval(report)

    assert "RBForge debugger eval" in text
    assert "families: 1" in text
    assert "debugger-use-rate: 100.0%" in text
    assert "root-cause-hit-rate: 100.0%" in text
    assert "avg-turn-reduction: 50.0%" in text


def test_eval_main_prints_json(tmp_path: Path, capsys: Any) -> None:
    cases = tmp_path / "cases.json"
    cases.write_text(
        json.dumps(
            {
                "cases": [
                    {
                        "id": "case-one",
                        "log": 'FAILED t.py::test_a\nFile "a.py", line 1\nValueError: bad',
                        "expected": {
                            "exception": "ValueError",
                            "file": "a.py",
                            "test": "t.py::test_a",
                        },
                        "baseline": {"turns": 4, "root_cause_hit": False},
                        "debugger": {
                            "used": True,
                            "turns": 2,
                            "reusable_debugger_created": False,
                        },
                    }
                ]
            }
        ),
        encoding="utf-8",
    )

    assert main(["debugger", "--cases", str(cases), "--format", "json"]) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["schema"] == "rbforge.eval.debugger.v1"


def test_default_debugger_eval_fixture_is_broad() -> None:
    report = build_debugger_eval(load_debugger_cases(DEFAULT_DEBUGGER_CASES))

    assert report["cases"] >= 15
    assert report["families"] >= 10

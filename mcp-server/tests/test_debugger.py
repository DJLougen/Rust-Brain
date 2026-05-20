from __future__ import annotations

from rbforge_core.debugger import debugger_signal_report


def test_debugger_signal_report_extracts_failure_shape() -> None:
    report = debugger_signal_report(
        "\n".join(
            [
                "FAILED tests/test_cache.py::test_lock_timeout",
                "Traceback (most recent call last):",
                '  File "app/cache.py", line 42, in get',
                "TimeoutError: lock wait exceeded",
                "AssertionError: expected cache hit",
            ]
        )
    )

    assert report["traceback_count"] == 1
    assert report["top_exception"] == "TimeoutError"
    assert report["exception_types"]["TimeoutError"] == 1
    assert report["suspect_files"] == {"app/cache.py": 1}
    assert report["failing_tests"] == ["tests/test_cache.py::test_lock_timeout"]
    assert report["debugger_signal_score"] > 0


def test_debugger_signal_report_handles_cli_system_exit() -> None:
    report = debugger_signal_report(
        "\n".join(
            [
                "ERROR tests/test_cli.py::test_format_json_flag",
                "Traceback (most recent call last):",
                '  File "rbforge_core/cli.py", line 29, in main',
                "SystemExit: 2",
            ]
        )
    )

    assert report["top_exception"] == "SystemExit"
    assert report["suspect_files"] == {"rbforge_core/cli.py": 1}

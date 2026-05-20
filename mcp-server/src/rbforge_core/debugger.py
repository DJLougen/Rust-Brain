"""Deterministic debugger signals for forged-tool training and evaluation."""

from __future__ import annotations

import re
from collections import Counter
from typing import Any

_EXCEPTION_RE = re.compile(
    r"\b([A-Za-z_][A-Za-z0-9_]*(?:Error|Exception)|SystemExit|KeyboardInterrupt)\b"
)
_FILE_RE = re.compile(r'File "([^"]+)", line (\d+)')
_TEST_RE = re.compile(r"\b(?:FAILED|ERROR)\s+([^\s:]+(?:::[^\s:]+)*)")


def debugger_signal_report(text: str) -> dict[str, Any]:
    """Extract compact debugging signals from logs, tracebacks, and test output."""
    lines = text.splitlines()
    exception_types = Counter(_EXCEPTION_RE.findall(text))
    suspect_files = Counter(match.group(1) for match in _FILE_RE.finditer(text))
    failing_tests = _TEST_RE.findall(text)
    error_lines = [line.strip() for line in lines if _looks_like_error_line(line)]
    traceback_count = text.count("Traceback")

    return {
        "traceback_count": traceback_count,
        "exception_types": dict(exception_types.most_common()),
        "top_exception": exception_types.most_common(1)[0][0] if exception_types else None,
        "suspect_files": dict(suspect_files.most_common(5)),
        "failing_tests": failing_tests[:10],
        "error_line_count": len(error_lines),
        "error_lines": error_lines[:10],
        "debugger_signal_score": _signal_score(
            traceback_count=traceback_count,
            exception_count=sum(exception_types.values()),
            failing_test_count=len(failing_tests),
            error_line_count=len(error_lines),
        ),
    }


def _looks_like_error_line(line: str) -> bool:
    lowered = line.lower()
    return any(
        marker in lowered
        for marker in (
            "error",
            "exception",
            "failed",
            "assert",
            "traceback",
            "timeout",
            "deadlock",
        )
    )


def _signal_score(
    *,
    traceback_count: int,
    exception_count: int,
    failing_test_count: int,
    error_line_count: int,
) -> float:
    raw = (
        min(traceback_count, 3) * 0.25
        + min(exception_count, 5) * 0.08
        + min(failing_test_count, 5) * 0.10
        + min(error_line_count, 10) * 0.03
    )
    return round(min(raw, 1.0), 4)

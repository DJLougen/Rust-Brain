from __future__ import annotations

from datetime import datetime, timedelta, timezone

from rbforge_core.compactor import (
    CompactionResult,
    DistillSummary,
    compact_memory,
    distill_section,
    identify_stale_sections,
)


def _make_section(
    name: str = "test",
    usage_count: int = 0,
    success_rate: float = 1.0,
    last_used: str | None = None,
) -> dict:
    if last_used is None:
        last_used = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    return {
        "name": name,
        "path": f"tools.custom.{name}",
        "metrics": {
            "usage_count": usage_count,
            "success_rate": success_rate,
            "last_used_at": last_used,
        },
    }


def test_identify_stale_no_stale_sections() -> None:
    now = datetime.now(timezone.utc)
    recent = now - timedelta(hours=1)
    sections = [
        _make_section(name="a", usage_count=10, success_rate=0.9, last_used=recent.isoformat().replace("+00:00", "Z")),
        _make_section(name="b", usage_count=20, success_rate=0.8, last_used=recent.isoformat().replace("+00:00", "Z")),
    ]
    stale = identify_stale_sections(sections, now=now)
    assert stale == []


def test_identify_stale_low_success_rate() -> None:
    now = datetime.now(timezone.utc)
    recent = now - timedelta(hours=1)
    sections = [
        _make_section(
            name="bad",
            usage_count=10,
            success_rate=0.1,
            last_used=recent.isoformat().replace("+00:00", "Z"),
        ),
    ]
    stale = identify_stale_sections(sections, now=now)
    assert len(stale) == 1
    assert "low_success_rate" in stale[0].get("_stale_reason", "")


def test_identify_stale_too_old() -> None:
    now = datetime.now(timezone.utc)
    old = now - timedelta(days=120)
    sections = [
        _make_section(
            name="old",
            usage_count=10,
            success_rate=0.5,
            last_used=old.isoformat().replace("+00:00", "Z"),
        ),
    ]
    stale = identify_stale_sections(sections, now=now, max_age_days=90)
    assert len(stale) == 1
    assert "stale_age" in stale[0].get("_stale_reason", "")


def test_distill_section_truncates_long_content() -> None:
    section = {
        "name": "long_tool",
        "path": "tools.custom.long_tool",
        "implementation": "x" * 1000,
        "metrics": {"usage_count": 5, "success_rate": 0.8},
    }
    summary = distill_section(section, max_summary_length=100)
    assert isinstance(summary, DistillSummary)
    assert summary.section_path == "tools.custom.long_tool"
    assert "implementation" in summary.preserved_keys


def test_distill_section_collapses_run_history() -> None:
    section = {
        "name": "test",
        "path": "tools.custom.test",
        "metrics": {"usage_count": 10, "success_rate": 0.9},
        "run_history": [{"ok": True, "used_at": "2026-01-01T00:00:00Z"}] * 25,
    }
    summary = distill_section(section)
    assert "run_history" in summary.preserved_keys


def test_compact_memory_dry_run_returns_result() -> None:
    class FakeStore:
        def context(self, *a, **kw):
            return {"sections": []}

    result = compact_memory(FakeStore(), dry_run=True)  # type: ignore[arg-type]
    assert isinstance(result, CompactionResult)
    assert result.sections_scanned == 0

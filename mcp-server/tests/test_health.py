from __future__ import annotations

from datetime import datetime, timedelta, timezone

from rbforge_core.health import (
    ComponentScore,
    HealthReport,
    _conflict_density_score,
    _contradiction_score,
    _graph_connectivity_score,
    _recency_score,
    compute_health_score,
)


def _make_section(
    name: str = "test",
    status: str = "validated",
    usage_count: int = 10,
    success_rate: float = 0.8,
    last_used: str | None = None,
    graph_relations: int = 0,
) -> dict:
    now = datetime.now(timezone.utc)
    if last_used is None:
        last_used = now.isoformat().replace("+00:00", "Z")
    return {
        "name": name,
        "status": status,
        "metrics": {
            "usage_count": usage_count,
            "success_rate": success_rate,
            "last_used_at": last_used,
        },
        "graph": {
            "node_type": "tool",
            "relations": [{"to": "tools.registry", "type": "registered_in"}]
            * graph_relations,
        },
    }


def test_recency_score_fresh_sections() -> None:
    now = datetime.now(timezone.utc)
    recent = datetime.now(timezone.utc) - timedelta(hours=1)
    sections = [_make_section(last_used=recent.isoformat().replace("+00:00", "Z"))]
    score = _recency_score(sections, now=now)
    assert score.value > 0.9
    assert score.name == "recency"
    assert score.weight == 0.35


def test_recency_score_stale_sections() -> None:
    now = datetime.now(timezone.utc)
    stale = datetime.now(timezone.utc) - timedelta(days=45)
    sections = [_make_section(last_used=stale.isoformat().replace("+00:00", "Z"))]
    score = _recency_score(sections, now=now)
    assert score.value < 0.4
    assert score.flag is not None


def test_conflict_density_no_conflicts() -> None:
    sections = [_make_section(name="a", status="validated"), _make_section(name="b", status="validated")]
    score = _conflict_density_score(sections)
    assert score.value == 1.0


def test_graph_connectivity_no_graph() -> None:
    sections = [_make_section()]
    score = _graph_connectivity_score(sections)
    assert score.value == 0.0


def test_contradiction_score_no_contradictions() -> None:
    sections = [_make_section(), _make_section(name="other")]
    score = _contradiction_score(sections)
    assert score.value == 1.0


def test_compute_health_score_produces_valid_report() -> None:
    sections = [
        _make_section(name="a", usage_count=10, success_rate=0.9),
        _make_section(name="b", usage_count=5, success_rate=0.7),
    ]
    report = compute_health_score(sections)
    assert isinstance(report, HealthReport)
    assert 0.0 <= report.composite_score <= 1.0
    assert len(report.component_scores) == 4
    assert isinstance(report.flags, list)
    assert isinstance(report.recommendations, list)
    assert report.section_count == 2


def test_health_score_recommends_compact_for_stale() -> None:
    now = datetime.now(timezone.utc)
    stale = datetime.now(timezone.utc) - timedelta(days=100)
    sections = [_make_section(last_used=stale.isoformat().replace("+00:00", "Z"))]
    report = compute_health_score(sections, now=now)
    assert any("compact" in rec.lower() for rec in report.recommendations)

"""Memory health scoring: composite scores from recency, conflicts, graph connectivity."""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from rbforge_core.rbmem import RbmemStore


def _parse_timestamp(ts: str) -> datetime | None:
    """Parse an ISO timestamp string, handling 'Z' suffix."""
    try:
        return datetime.fromisoformat(ts.replace("Z", "+00:00"))
    except (ValueError, TypeError):
        return None


@dataclass(frozen=True)
class ComponentScore:
    """Score for a single health dimension."""

    name: str
    value: float  # 0.0 to 1.0
    weight: float  # contribution weight (sums to 1.0 across components)
    flag: str | None = None


@dataclass
class HealthReport:
    """Composite health report for an RBMEM file."""

    composite_score: float
    component_scores: list[ComponentScore]
    flags: list[str] = field(default_factory=list)
    recommendations: list[str] = field(default_factory=list)
    section_count: int = 0
    graph_edges: int = 0


def _recency_score(sections: list[dict[str, Any]], now: datetime | None = None) -> ComponentScore:
    """Compute a recency score based on how recently sections were used.

    Returns 1.0 if all sections are fresh, lower scores for stale data.
    """
    if now is None:
        now = datetime.now(timezone.utc)

    ages: list[float] = []
    for section in sections:
        last_used = section.get("metrics", {}).get("last_used_at")
        if isinstance(last_used, str):
            ts = _parse_timestamp(last_used)
            if ts is not None:
                ages.append((now - ts).total_seconds())
        else:
            # If no timestamp, assume moderate staleness
            ages.append(86400.0)  # 1 day default

    if not ages:
        return ComponentScore(
            name="recency",
            value=1.0,
            weight=0.35,
            flag=None,
        )

    avg_age_seconds = sum(ages) / len(ages)
    # Score: 1.0 at 0 days, 0.5 at 7 days, approaching 0 at 30 days
    if avg_age_seconds <= 604800:  # 7 days
        score = 1.0 - (avg_age_seconds / 604800) * 0.5
    else:
        score = max(0.0, 0.5 - ((avg_age_seconds - 604800) / (30 * 86400 - 604800)) * 0.5)

    flag = None
    if score < 0.3:
        flag = "MEMORY_STALE"
    elif score < 0.6:
        flag = "MEMORY_AGING"

    return ComponentScore(
        name="recency",
        value=round(score, 4),
        weight=0.35,
        flag=flag,
    )


def _conflict_density_score(sections: list[dict[str, Any]]) -> ComponentScore:
    """Compute a conflict density score based on contradictory content.

    Detects sections with conflicting status values or contradictory metrics.
    """
    if not sections:
        return ComponentScore(
            name="conflict_density",
            value=1.0,
            weight=0.25,
            flag=None,
        )

    conflict_count = 0
    total_comparisons = 0

    # Check for conflicting status across related sections
    status_by_name: dict[str, list[str]] = {}
    for section in sections:
        name = section.get("name") or section.get("path", "")
        status = section.get("status", "")
        if name and status:
            status_by_name.setdefault(name, []).append(status)

    for name, statuses in status_by_name.items():
        unique = set(statuses)
        if len(unique) > 1:
            conflict_count += 1
        total_comparisons += 1

    # Check metrics contradictions (usage_count mismatch across versions)
    metrics_by_name: dict[str, list[dict[str, Any]]] = {}
    for section in sections:
        name = section.get("name") or section.get("path", "")
        metrics = section.get("metrics", {})
        if name and isinstance(metrics, dict):
            metrics_by_name.setdefault(name, []).append(metrics)

    for name, metrics_list in metrics_by_name.items():
        if len(metrics_list) > 1:
            counts = [
                m.get("usage_count", 0)
                for m in metrics_list
                if isinstance(m.get("usage_count"), (int, float))
            ]
            if counts and max(counts) - min(counts) > 100:
                conflict_count += 1
            total_comparisons += 1

    if total_comparisons == 0:
        density = 0.0
    else:
        density = conflict_count / total_comparisons

    # Score inversely proportional to conflict density
    score = max(0.0, 1.0 - density)
    flag = None
    if density > 0.3:
        flag = "HIGH_CONFLICT_DENSITY"
    elif density > 0.1:
        flag = "MODERATE_CONFLICTS"

    return ComponentScore(
        name="conflict_density",
        value=round(score, 4),
        weight=0.25,
        flag=flag,
    )


def _graph_connectivity_score(sections: list[dict[str, Any]]) -> ComponentScore:
    """Compute a graph connectivity score based on section graph edges.

    A well-connected graph (many cross-references) scores higher.
    """
    if len(sections) < 2:
        edge_count = 0
        node_count = max(len(sections), 1)
    else:
        node_count = len(sections)
        edge_count = 0
        for section in sections:
            graph = section.get("graph", {})
            if isinstance(graph, dict):
                relations = graph.get("relations", [])
                if isinstance(relations, list):
                    edge_count += len(relations)

    if node_count <= 1:
        connectivity = 0.0
    else:
        # Max possible edges is node_count * (node_count - 1)
        max_edges = node_count * (node_count - 1)
        connectivity = edge_count / max_edges if max_edges > 0 else 0.0

    flag = None
    if connectivity < 0.05 and node_count > 5:
        flag = "LOW_GRAPH_CONNECTIVITY"
    elif connectivity < 0.15:
        flag = "WEAK_CONNECTIVITY"

    return ComponentScore(
        name="graph_connectivity",
        value=round(connectivity, 4),
        weight=0.20,
        flag=flag,
    )


def _contradiction_score(sections: list[dict[str, Any]]) -> ComponentScore:
    """Score based on contradictions between related sections.

    Looks for sections that reference each other but have inconsistent data.
    """
    if len(sections) < 2:
        return ComponentScore(
            name="contradictions",
            value=1.0,
            weight=0.20,
            flag=None,
        )

    contradiction_count = 0
    relationship_count = 0

    # Extract graph relationships between sections
    for section in sections:
        graph = section.get("graph", {})
        if not isinstance(graph, dict):
            continue
        relations = graph.get("relations", [])
        if not isinstance(relations, list):
            continue
        for relation in relations:
            if not isinstance(relation, dict):
                continue
            target = relation.get("to", "")
            rel_type = relation.get("type", "")
            if target and rel_type:
                relationship_count += 1
                # Check if target section exists and has conflicting data
                target_name = target.rsplit(".", 1)[-1]
                for other in sections:
                    if other.get("name") == target_name or other.get("path") == target:
                        # Check for contradictory metrics
                        our_metrics = section.get("metrics", {})
                        other_metrics = other.get("metrics", {})
                        if isinstance(our_metrics, dict) and isinstance(other_metrics, dict):
                            our_count = our_metrics.get("usage_count", 0)
                            other_count = other_metrics.get("usage_count", 0)
                            if isinstance(our_count, (int, float)) and isinstance(
                                other_count, (int, float)
                            ):
                                if abs(our_count - other_count) > 50:
                                    contradiction_count += 1
                        break

    if relationship_count == 0:
        score = 1.0
    else:
        score = max(0.0, 1.0 - contradiction_count / relationship_count)

    flag = None
    if score < 0.5:
        flag = "SEVERE_CONTRADICTIONS"
    elif score < 0.8:
        flag = "CONTRADICTIONS_FOUND"

    return ComponentScore(
        name="contradictions",
        value=round(score, 4),
        weight=0.20,
        flag=flag,
    )


def compute_health_score(
    sections: list[dict[str, Any]],
    now: datetime | None = None,
) -> HealthReport:
    """Compute a composite health score for a set of RBMEM sections.

    Combines recency, conflict density, graph connectivity, and
    contradictions into a single composite score (0.0-1.0).

    Args:
        sections: List of section records from RBMEM context query.
        now: Optional current timestamp for recency calculations.

    Returns:
        A :class:`HealthReport` with composite score, component scores,
        flags, and recommendations.
    """
    recency = _recency_score(sections, now=now)
    conflicts = _conflict_density_score(sections)
    connectivity = _graph_connectivity_score(sections)
    contradictions = _contradiction_score(sections)

    component_scores = [recency, conflicts, connectivity, contradictions]

    # Weighted composite
    composite = sum(cs.value * cs.weight for cs in component_scores)
    composite = round(composite, 4)

    # Collect all flags
    flags = [cs.flag for cs in component_scores if cs.flag is not None]

    # Generate recommendations
    recommendations: list[str] = []
    if recency.value < 0.5:
        recommendations.append(
            "Consider running 'rbforge doctor compact' to remove stale sections"
        )
    if conflicts.value < 0.5:
        recommendations.append(
            "High conflict density detected; review section status values for contradictions"
        )
    if connectivity.value < 0.1 and len(sections) > 5:
        recommendations.append(
            "Low graph connectivity; consider establishing cross-references between tools"
        )
    if contradictions.value < 0.5:
        recommendations.append(
            "Contradictions found between related sections; run 'rbforge doctor compact' to resolve"
        )

    # Count graph edges
    graph_edges = 0
    for section in sections:
        graph = section.get("graph", {})
        if isinstance(graph, dict):
            relations = graph.get("relations", [])
            if isinstance(relations, list):
                graph_edges += len(relations)

    return HealthReport(
        composite_score=composite,
        component_scores=component_scores,
        flags=flags,
        recommendations=recommendations,
        section_count=len(sections),
        graph_edges=graph_edges,
    )


def health_report_from_store(
    store: RbmemStore,
) -> HealthReport:
    """Compute a health report from an RBMEM store.

    Convenience wrapper that queries the store for all sections and
    delegates to :func:`compute_health_score`.

    Args:
        store: An :class:`RbmemStore` instance.

    Returns:
        A :class:`HealthReport` computed from the store's sections.
    """
    try:
        payload = store.context("tools", resolve=True, minified=False, graph_depth=1)
    except Exception:  # noqa: BLE001 - diagnostics fallback
        payload = {"sections": []}

    sections: list[dict[str, Any]] = []
    for section in payload.get("sections", []):
        content = section.get("content")
        if isinstance(content, dict):
            content = content
        elif isinstance(content, str):
            try:
                content = json.loads(content)
            except (json.JSONDecodeError, TypeError):
                content = {}
        else:
            content = {}
        content["_section_path"] = section.get("path", "")
        sections.append(content)

    return compute_health_score(sections)

"""Temporal reasoning for RBMEM: time-windowed queries, trend analysis, usage patterns."""

from __future__ import annotations

import statistics
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from enum import Enum
from typing import Any

from rbforge_core.health import _parse_timestamp


class TrendDirection(Enum):
    """Direction of a metric trend."""

    INCREASING = "increasing"
    DECREASING = "decreasing"
    STABLE = "stable"
    INSUFFICIENT_DATA = "insufficient_data"


@dataclass
class DataPoint:
    """A single point in a temporal dataset."""

    timestamp: str
    value: float
    label: str = ""


@dataclass
class TemporalResult:
    """Result of a temporal analysis operation."""

    section_path: str
    data_points: list[DataPoint]
    trend: TrendDirection
    avg_value: float
    min_value: float
    max_value: float
    patterns: list[str] = field(default_factory=list)
    window_start: str = ""
    window_end: str = ""


def utc_now_iso() -> str:
    """Return the current UTC time as an ISO-8601 string."""
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _extract_timestamps(
    section: dict[str, Any],
) -> list[tuple[str, datetime]]:
    """Extract (iso_string, datetime) pairs from a section record.

    Looks at:
    - run_history entries (used_at field)
    - top-level timestamp_policy timestamps if present
    """
    results: list[tuple[str, datetime]] = []
    history = section.get("run_history", [])
    if isinstance(history, list):
        for entry in history:
            if not isinstance(entry, dict):
                continue
            used_at = entry.get("used_at")
            if isinstance(used_at, str):
                ts = _parse_timestamp(used_at)
                if ts is not None:
                    results.append((used_at, ts))

    # Also check metrics timestamps
    metrics = section.get("metrics", {})
    if isinstance(metrics, dict):
        for key in ("last_used_at", "created_at"):
            ts_str = metrics.get(key)
            if isinstance(ts_str, str):
                ts = _parse_timestamp(ts_str)
                if ts is not None:
                    results.append((ts_str, ts))

    return results


def temporal_query(
    section: dict[str, Any],
    window_start: datetime | None = None,
    window_end: datetime | None = None,
) -> TemporalResult:
    """Perform a time-windowed query on a section's historical data.

    Extracts timestamps and usage-count deltas from run_history entries
    and returns the data points that fall within the specified window.

    Args:
        section: A section record from RBMEM.
        window_start: Start of the time window (inclusive). Defaults to 24h ago.
        window_end: End of the time window (exclusive). Defaults to now.

    Returns:
        A :class:`TemporalResult` with filtered data points.
    """
    if window_end is None:
        window_end = datetime.now(timezone.utc)
    if window_start is None:
        window_start = window_end - timedelta(hours=24)

    all_entries = _extract_timestamps(section)
    data_points: list[DataPoint] = []

    for iso_str, ts in all_entries:
        if window_start <= ts <= window_end:
            # Extract value from the history entry or metrics
            metrics = section.get("metrics", {})
            if isinstance(metrics, dict):
                value = float(metrics.get("usage_count", 0))
            else:
                value = 1.0
            data_points.append(
                DataPoint(timestamp=iso_str, value=value, label=ts.isoformat())
            )

    values = [dp.value for dp in data_points]
    trend = _compute_trend(values)

    return TemporalResult(
        section_path=section.get("_section_path", section.get("path", "unknown")),
        data_points=data_points,
        trend=trend,
        avg_value=statistics.mean(values) if values else 0.0,
        min_value=min(values) if values else 0.0,
        max_value=max(values) if values else 0.0,
        window_start=window_start.isoformat(),
        window_end=window_end.isoformat(),
    )


def trend_analysis(
    sections: list[dict[str, Any]],
    metric: str = "usage_count",
    window_hours: int = 168,  # 1 week default
) -> list[TemporalResult]:
    """Compute trend analysis across multiple sections for a given metric.

    For each section, extracts the specified metric from run_history
    entries and computes the trend direction using simple linear
    regression slope classification.

    Args:
        sections: Section records to analyze.
        metric: The metric name to track over time (e.g. "usage_count").
        window_hours: Time window in hours for the analysis.

    Returns:
        List of :class:`TemporalResult` objects, one per section, sorted
        by the magnitude of their trend slope.
    """
    window_end = datetime.now(timezone.utc)
    window_start = window_end - timedelta(hours=window_hours)

    results: list[TemporalResult] = []

    for section in sections:
        result = temporal_query(section, window_start=window_start, window_end=window_end)

        # Enhance with pattern detection
        patterns = _detect_usage_patterns(
            result.data_points, metric, window_hours
        )
        result.patterns = patterns

        results.append(result)

    # Sort by absolute trend magnitude (most volatile first)
    results.sort(key=lambda r: abs(_trend_magnitude(r.trend)))

    return results


def usage_pattern(section: dict[str, Any]) -> list[str]:
    """Identify usage patterns from a section's history.

    Detects patterns such as:
    - "bursty": many runs in short intervals
    - "steady": roughly constant usage rate
    - "declining": decreasing usage over time
    - "new": very few historical entries
    - "active": frequent recent usage

    Args:
        section: A section record from RBMEM.

    Returns:
        List of pattern name strings.
    """
    patterns: list[str] = []
    history = section.get("run_history", [])
    metrics = section.get("metrics", {})

    if not isinstance(history, list):
        history = []

    total_runs = len(history)
    usage_count = int(metrics.get("usage_count", 0)) if isinstance(metrics, dict) else 0
    success_rate = float(metrics.get("success_rate", 0.0)) if isinstance(metrics, dict) else 0.0

    # Extract timestamps for interval analysis
    timestamps: list[datetime] = []
    for entry in history:
        if not isinstance(entry, dict):
            continue
        ts_str = entry.get("used_at")
        if isinstance(ts_str, str):
            ts = _parse_timestamp(ts_str)
            if ts is not None:
                timestamps.append(ts)

    timestamps.sort()

    if total_runs == 0:
        patterns.append("inactive")
        return patterns

    if total_runs <= 3:
        patterns.append("new")

    if total_runs >= 10 and len(timestamps) >= 2:
        # Check for bursty pattern: many runs within short windows
        intervals = []
        for i in range(1, len(timestamps)):
            delta = (timestamps[i] - timestamps[i - 1]).total_seconds()
            intervals.append(delta)

        if intervals:
            avg_interval = statistics.mean(intervals)
            if avg_interval < 3600:  # less than 1 hour average
                patterns.append("bursty")
            elif avg_interval < 86400:  # less than 1 day
                patterns.append("steady")
            else:
                patterns.append("irregular")

    # Check for declining usage.
    # Compare event density (events per day) in the last 90 days vs
    # the 90-day period before that (days 91-180 ago).  If the recent
    # window has significantly fewer events, flag as declining.
    # This catches gradual decline, bursty-then-quiet, and sudden stops.
    if len(timestamps) >= 6:
        now_ts = timestamps[-1]
        # Recent window: last 90 days
        recent = [
            t for t in timestamps
            if now_ts - t <= timedelta(days=90)
        ]
        # Previous window: days 91-180 ago
        previous = [
            t for t in timestamps
            if timedelta(days=90) < now_ts - t <= timedelta(days=180)
        ]

        if len(previous) >= 2 and len(recent) < len(previous) * 0.5:
            patterns.append("declining")
        elif len(previous) >= 2 and len(recent) <= 1:
            # Sudden drop: had 2+ events in previous window but only
            # 0-1 in recent window — severe declining
            if "declining" not in patterns:
                patterns.append("declining")

    # If the last event is more than 60 days old and we have
    # at least 1 event, the tool has gone dormant — flag declining.
    # This catches cases with sparse temporal data (1-5 timestamps)
    # where the windowed density check above is not applicable.
    if timestamps:
        last_event_ago = datetime.now(timezone.utc) - timestamps[-1]
        if last_event_ago > timedelta(days=60) and "declining" not in patterns:
            patterns.append("declining")
    if success_rate < 0.3 and usage_count > 5:
        patterns.append("unreliable")

    if usage_count > 50 and success_rate > 0.9:
        patterns.append("reliable_active")

    if not patterns:
        patterns.append("normal")

    return patterns


def _compute_trend(values: list[float]) -> TrendDirection:
    """Classify the trend direction of a list of values.

    Uses a simple linear regression slope for classification.

    Args:
        values: Sequence of numeric values in chronological order.

    Returns:
        The :class:`TrendDirection` for the data.
    """
    if len(values) < 2:
        return TrendDirection.INSUFFICIENT_DATA

    n = len(values)
    x_mean = (n - 1) / 2.0
    y_mean = statistics.mean(values)

    numerator = sum((i - x_mean) * (v - y_mean) for i, v in enumerate(values))
    denominator = sum((i - x_mean) ** 2 for i in range(n))

    if denominator == 0:
        return TrendDirection.STABLE

    slope = numerator / denominator
    magnitude = abs(slope) / max(y_mean, 1.0)

    if magnitude < 0.01:
        return TrendDirection.STABLE
    elif slope > 0:
        return TrendDirection.INCREASING
    else:
        return TrendDirection.DECREASING


def _trend_magnitude(trend: TrendDirection) -> float:
    """Return a numeric magnitude for a trend direction."""
    magnitudes = {
        TrendDirection.INCREASING: 1.0,
        TrendDirection.DECREASING: -1.0,
        TrendDirection.STABLE: 0.0,
        TrendDirection.INSUFFICIENT_DATA: 0.0,
    }
    return magnitudes.get(trend, 0.0)


def _detect_usage_patterns(
    data_points: list[DataPoint],
    metric: str,
    window_hours: int,
) -> list[str]:
    """Detect usage patterns within a set of data points."""
    patterns: list[str] = []

    if len(data_points) < 2:
        return ["insufficient_data"]

    values = [dp.value for dp in data_points]
    trend = _compute_trend(values)

    if trend == TrendDirection.INCREASING:
        patterns.append("growing_usage")
    elif trend == TrendDirection.DECREASING:
        patterns.append("fading_usage")
    elif trend == TrendDirection.STABLE:
        if statistics.stdev(values) < 1.0 and len(values) >= 3:
            patterns.append("steady_usage")

    # Check for high-frequency bursts
    if len(data_points) >= 5:
        first_quarter = data_points[: len(data_points) // 4]
        last_quarter = data_points[-(len(data_points) // 4) :]
        first_sum = sum(dp.value for dp in first_quarter)
        last_sum = sum(dp.value for dp in last_quarter)
        if last_sum > first_sum * 2:
            patterns.append("accelerating")

    return patterns

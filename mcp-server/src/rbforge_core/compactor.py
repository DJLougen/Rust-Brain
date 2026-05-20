"""Memory compaction/distillation: detect stale sections and summarize them."""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from rbforge_core.health import _parse_timestamp, compute_health_score
from rbforge_core.rbmem import RbmemStore
from rbforge_core.registry import _archive_reason


def _config_staleness_threshold(
    memory_path: str | Path = "memory.rbmem",
    *,
    max_age_days: float = 90.0,
    min_usage_count: int = 5,
    min_success_rate: float = 0.3,
) -> dict[str, Any]:
    """Load optional staleness config from the memory file metadata.

    Checks for an ``rbforge.compact`` section that may override defaults.
    """
    config_path = Path(memory_path) / ".rbforge_compact_config"
    if config_path.exists():
        try:
            return json.loads(config_path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            pass

    return {
        "max_age_days": max_age_days,
        "min_usage_count": min_usage_count,
        "min_success_rate": min_success_rate,
    }


def identify_stale_sections(
    sections: list[dict[str, Any]],
    *,
    max_age_days: float = 90.0,
    min_usage_count: int = 5,
    min_success_rate: float = 0.3,
    now: datetime | None = None,
) -> list[dict[str, Any]]:
    """Find low-value sections by usage recency and conflict metrics.

    A section is considered stale if ANY of these conditions hold:
    - Last used more than ``max_age_days`` ago (and usage_count >= min_usage_count)
    - Success rate below ``min_success_rate`` with at least ``min_usage_count`` runs
    - Has an archive reason from the registry audit logic

    Args:
        sections: Section records from RBMEM context query.
        max_age_days: Maximum acceptable age in days.
        min_usage_count: Minimum usage count before staleness is considered.
        min_success_rate: Minimum acceptable success rate.
        now: Optional current timestamp.

    Returns:
        List of stale section records, each annotated with a ``_stale_reason`` key.
    """
    if now is None:
        now = datetime.now(timezone.utc)

    stale: list[dict[str, Any]] = []

    for section in sections:
        reason = _stale_reason(
            section,
            max_age_days=max_age_days,
            min_usage_count=min_usage_count,
            min_success_rate=min_success_rate,
            now=now,
        )
        if reason is not None:
            stale_section = dict(section)
            stale_section["_stale_reason"] = reason
            stale.append(stale_section)

    return stale


def _stale_reason(
    section: dict[str, Any],
    *,
    max_age_days: float,
    min_usage_count: int,
    min_success_rate: float,
    now: datetime,
) -> str | None:
    """Determine a staleness reason for a single section."""
    # Check archive reason from registry audit
    archive_reason = _archive_reason(section.get("metrics", {}))
    if archive_reason is not None:
        return f"registry_archive: {archive_reason}"

    metrics = section.get("metrics", {})
    if not isinstance(metrics, dict):
        return None

    usage_count = int(metrics.get("usage_count", 0))
    success_rate = float(metrics.get("success_rate", 1.0))
    last_used = metrics.get("last_used_at")

    # Low success rate with enough usage
    if usage_count >= min_usage_count and success_rate < min_success_rate:
        return f"low_success_rate: {success_rate:.2f} over {usage_count} runs"

    # Too old with meaningful usage
    if isinstance(last_used, str) and usage_count >= min_usage_count:
        ts = _parse_timestamp(last_used)
        if ts is not None:
            age = now - ts
            if age.total_seconds() / 86400 > max_age_days:
                return f"stale_age: {age.days} days since last use"

    return None


@dataclass
class DistillSummary:
    """Condensed summary of a distillation operation."""

    section_path: str
    original_size_chars: int
    distilled_size_chars: int
    original_content: str
    distilled_content: str
    preserved_keys: list[str]
    removed_keys: list[str]


def distill_section(
    section: dict[str, Any],
    *,
    max_summary_length: int = 500,
) -> DistillSummary:
    """Summarize a section's content into a condensed form.

    Preserves top-level keys but truncates long values. Run-history
    entries are collapsed to aggregate statistics.

    Args:
        section: The section record to distill.
        max_summary_length: Maximum length of the distilled content string.

    Returns:
        A :class:`DistillSummary` describing the transformation.
    """
    original = json.dumps(section, indent=2, sort_keys=True)
    preserved_keys: list[str] = []
    removed_keys: list[str] = []
    distilled: dict[str, Any] = {}

    for key, value in section.items():
        if key.startswith("_"):
            removed_keys.append(key)
            continue

        if key == "metrics":
            distilled[key] = {
                "usage_count": value.get("usage_count", 0),
                "success_rate": value.get("success_rate", 0.0),
                "last_used_at": value.get("last_used_at"),
            }
            preserved_keys.append(key)
            continue

        if key == "run_history" and isinstance(value, list):
            # Collapse to summary stats
            ok_count = sum(1 for r in value if isinstance(r, dict) and r.get("ok"))
            total = len(value)
            distilled[key] = {
                "total_runs": total,
                "successes": ok_count,
                "failures": total - ok_count,
                "entries_truncated": total,
            }
            preserved_keys.append(key)
            continue

        if isinstance(value, str) and len(value) > max_summary_length:
            distilled[key] = value[: max_summary_length // 2] + f"\n...({len(value)} chars total)"
            preserved_keys.append(key)
            continue

        if isinstance(value, (dict, list)) and (
            (isinstance(value, dict) and len(value) > 10)
            or (isinstance(value, list) and len(value) > 20)
        ):
            distilled[key] = f"[{len(value)} items]"
            preserved_keys.append(key)
            continue

        distilled[key] = value
        preserved_keys.append(key)

    distilled_content = json.dumps(distilled, indent=2, sort_keys=True)
    # Support both _section_path and path as the section identifier
    section_path = section.get("_section_path") or section.get("path") or "unknown"
    return DistillSummary(
        section_path=section_path,
        original_size_chars=len(original),
        distilled_size_chars=len(distilled_content),
        original_content=original,
        distilled_content=distilled_content,
        preserved_keys=preserved_keys,
        removed_keys=removed_keys,
    )


@dataclass
class CompactionResult:
    """Result of a full memory compaction run."""

    sections_scanned: int
    stale_sections_found: int
    sections_distilled: int
    total_original_bytes: int
    total_distilled_bytes: int
    stale_section_paths: list[str]
    distillation_summaries: list[DistillSummary]


def compact_memory(
    store: RbmemStore,
    *,
    dry_run: bool = True,
    max_age_days: float = 90.0,
    min_usage_count: int = 5,
    min_success_rate: float = 0.3,
) -> CompactionResult:
    """Orchestrate stale detection and distillation across all sections.

    Args:
        store: The RBMEM store to compact.
        dry_run: If True, only report stale sections without modifying.
        max_age_days: Pass-through to :func:`identify_stale_sections`.
        min_usage_count: Pass-through to :func:`identify_stale_sections`.
        min_success_rate: Pass-through to :func:`identify_stale_sections`.

    Returns:
        A :class:`CompactionResult` summarising the operation.
    """
    try:
        payload = store.context("tools", resolve=True, minified=False, graph_depth=1)
    except Exception:  # noqa: BLE001
        payload = {"sections": []}

    sections: list[dict[str, Any]] = []
    for section in payload.get("sections", []):
        content = section.get("content")
        if isinstance(content, dict):
            pass
        elif isinstance(content, str):
            try:
                content = json.loads(content)
            except (json.JSONDecodeError, TypeError):
                content = {}
        else:
            content = {}
        content["_section_path"] = section.get("path", "")
        sections.append(content)

    stale = identify_stale_sections(
        sections,
        max_age_days=max_age_days,
        min_usage_count=min_usage_count,
        min_success_rate=min_success_rate,
    )

    summaries: list[DistillSummary] = []
    stale_paths: list[str] = []

    for stale_section in stale:
        summary = distill_section(stale_section)
        stale_paths.append(summary.section_path)
        summaries.append(summary)

    if not dry_run:
        for summary in summaries:
            try:
                store.update_section(
                    summary.section_path,
                    "json",
                    json.loads(summary.distilled_content),
                )
            except Exception:  # noqa: BLE001 - non-fatal for compaction
                pass

    total_original = sum(s.original_size_chars for s in summaries)
    total_distilled = sum(s.distilled_size_chars for s in summaries)

    return CompactionResult(
        sections_scanned=len(sections),
        stale_sections_found=len(stale),
        sections_distilled=len(summaries),
        total_original_bytes=total_original,
        total_distilled_bytes=total_distilled,
        stale_section_paths=stale_paths,
        distillation_summaries=summaries,
    )

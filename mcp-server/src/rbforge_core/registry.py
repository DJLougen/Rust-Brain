"""Registry maintenance and deprecation policy."""

from __future__ import annotations

from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

from rbforge_core.models import utc_now_iso
from rbforge_core.rbmem import RbmemStore


def audit_registry(
    memory_path: str | Path = "memory.rbmem",
    *,
    dry_run: bool = True,
    rbmem_cli: str | None = None,
    store: Any | None = None,
) -> list[dict[str, Any]]:
    store = store or RbmemStore(memory_path, rbmem_cli=rbmem_cli)
    recommendations: list[dict[str, Any]] = []
    for item in store.read_registry():
        name = item["name"]
        record = store.load_tool_record(name)
        reason = _archive_reason(record)
        if reason is None:
            continue
        recommendation = {"tool": name, "action": "archive", "reason": reason}
        recommendations.append(recommendation)
        if not dry_run:
            archived = dict(record)
            archived["status"] = "archived"
            archived["archived_at"] = utc_now_iso()
            archived["archive_reason"] = reason
            store.update_section(f"tools.archive.{name}", "json", archived)
            tombstone = dict(record)
            tombstone["status"] = "archived"
            tombstone["archived_at"] = archived["archived_at"]
            tombstone["archive_reason"] = reason
            store.update_section(f"tools.custom.{name}", "json", tombstone)
            _rewrite_registry_without_archived(store, name)
    return recommendations


def _archive_reason(record: dict[str, Any]) -> str | None:
    metrics = record.get("metrics", {})
    usage_count = int(metrics.get("usage_count", 0))
    success_rate = float(metrics.get("success_rate", 1.0))
    if usage_count >= 20 and success_rate < 0.3:
        return "success_rate below 0.3 over at least 20 runs"
    last_used = metrics.get("last_used_at")
    if isinstance(last_used, str):
        try:
            last = datetime.fromisoformat(last_used.replace("Z", "+00:00"))
        except ValueError:
            return None
        if datetime.now(timezone.utc) - last > timedelta(days=90):
            return "unused for more than 90 days"
    return None


def _rewrite_registry_without_archived(store: Any, archived_name: str) -> None:
    registry = [
        item
        for item in store.read_registry()
        if isinstance(item, dict) and item.get("name") != archived_name
    ]
    store.update_section(
        "tools.registry",
        "json",
        {
            "schema": "rbforge.tool_registry.v1",
            "updated_by": "RBForge",
            "tools": registry,
        },
    )

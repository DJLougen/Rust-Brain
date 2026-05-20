"""Versioning and rollback for RBMEM sections: snapshot, restore, diff."""

from __future__ import annotations

import difflib
import hashlib
import json
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def _ensure_snapshot_dir(base_dir: Path | str = ".") -> Path:
    """Create and return the .rbforge_snapshots directory."""
    snapshot_dir = Path(base_dir) / ".rbforge_snapshots"
    snapshot_dir.mkdir(parents=True, exist_ok=True)
    return snapshot_dir


def _section_hash(content: Any) -> str:
    """Compute a SHA-256 hash of section content for change detection."""
    text = json.dumps(content, sort_keys=True, default=str)
    return hashlib.sha256(text.encode("utf-8")).hexdigest()[:16]


def utc_now_iso() -> str:
    """Return the current UTC time as an ISO-8601 string."""
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


@dataclass(frozen=True)
class Snapshot:
    """A point-in-time snapshot of a section's state."""

    snapshot_id: str
    section_path: str
    content_hash: str
    content: dict[str, Any]
    created_at: str
    source_file: str


@dataclass
class SnapshotDiff:
    """Diff between two snapshots."""

    snapshot_id_1: str
    snapshot_id_2: str
    section_path: str
    diff_lines: list[str]
    fields_changed: list[str]
    added_keys: list[str]
    removed_keys: list[str]


@dataclass
class SnapshotList:
    """Paginated list of snapshots for a given section."""

    section_path: str
    snapshots: list[Snapshot]
    total: int


def create_snapshot(
    section_path: str,
    content: dict[str, Any],
    *,
    base_dir: Path | str = ".",
) -> Snapshot:
    """Create a snapshot of a section's current state.

    The snapshot is persisted to disk in the ``.rbforge_snapshots/``
    directory under the caller's ``base_dir``.

    Args:
        section_path: The RBMEM section path being snapshotted.
        content: The full section content dictionary.
        base_dir: Root directory for the snapshot store.

    Returns:
        A :class:`Snapshot` with the snapshot data and persisted to disk.
    """
    snapshot_dir = _ensure_snapshot_dir(base_dir)
    content_hash = _section_hash(content)

    # Generate a unique snapshot ID from timestamp + hash prefix
    timestamp = utc_now_iso()
    snapshot_id = f"{section_path.replace('.', '_')}_{timestamp.replace(':', '-').replace('+', '_')}_{content_hash}"

    snapshot = Snapshot(
        snapshot_id=snapshot_id,
        section_path=section_path,
        content_hash=content_hash,
        content=content,
        created_at=timestamp,
        source_file=str(snapshot_dir / f"{snapshot_id}.json"),
    )

    snapshot_path = Path(snapshot.source_file)
    snapshot_path.write_text(
        json.dumps(
            {
                "snapshot_id": snapshot_id,
                "section_path": section_path,
                "content_hash": content_hash,
                "content": content,
                "created_at": timestamp,
            },
            indent=2,
            sort_keys=True,
        ),
        encoding="utf-8",
    )

    return snapshot


def list_snapshots(
    section_path: str | None = None,
    *,
    base_dir: Path | str = ".",
) -> SnapshotList:
    """Return all snapshots, optionally filtered by section path.

    Snapshots are sorted by creation timestamp (newest first).

    Args:
        section_path: Optional section path to filter by.
        base_dir: Root directory for the snapshot store.

    Returns:
        A :class:`SnapshotList` with up to 100 snapshots.
    """
    snapshot_dir = _ensure_snapshot_dir(base_dir)
    snapshots: list[Snapshot] = []

    if not snapshot_dir.exists():
        return SnapshotList(
            section_path=section_path or "",
            snapshots=[],
            total=0,
        )

    for json_file in snapshot_dir.glob("*.json"):
        try:
            data = json.loads(json_file.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            continue

        snap = Snapshot(
            snapshot_id=data["snapshot_id"],
            section_path=data["section_path"],
            content_hash=data["content_hash"],
            content=data["content"],
            created_at=data["created_at"],
            source_file=str(json_file),
        )

        if section_path is not None and snap.section_path != section_path:
            continue

        snapshots.append(snap)

    snapshots.sort(key=lambda s: s.created_at, reverse=True)

    return SnapshotList(
        section_path=section_path or "",
        snapshots=snapshots,
        total=len(snapshots),
    )


def restore_snapshot(
    snapshot_id: str,
    *,
    base_dir: Path | str = ".",
    store: Any | None = None,
) -> Snapshot | None:
    """Roll back a section to the state recorded in a snapshot.

    The snapshot JSON is loaded from disk, its content is restored to
    the section's original path via the provided ``store``, and the
    snapshot file is kept for auditability.

    Args:
        snapshot_id: The unique snapshot identifier to restore.
        base_dir: Root directory for the snapshot store.
        store: Optional RBMEM store to write the restored content.

    Returns:
        The :class:`Snapshot` that was restored, or ``None`` if not found.
    """
    snapshot_dir = _ensure_snapshot_dir(base_dir)
    snapshot_file = snapshot_dir / f"{snapshot_id}.json"

    if not snapshot_file.exists():
        return None

    data = json.loads(snapshot_file.read_text(encoding="utf-8"))
    content = data["content"]

    # If a store is provided, persist the restored content
    if store is not None:
        store_path = data.get("section_path", "")
        if store_path:
            store.update_section(store_path, "json", content)

    return Snapshot(
        snapshot_id=data["snapshot_id"],
        section_path=data["section_path"],
        content_hash=data["content_hash"],
        content=content,
        created_at=data["created_at"],
        source_file=str(snapshot_file),
    )


def diff_snapshots(
    id1: str,
    id2: str,
    *,
    base_dir: str | Path = ".",
) -> SnapshotDiff | None:
    """Compute a diff between two snapshots.

    Loads both snapshots from the default snapshot directory, compares
    their content, and returns a :class:`SnapshotDiff` summarising
    the structural differences.

    Args:
        id1: The older snapshot ID (baseline).
        id2: The newer snapshot ID (current).
        base_dir: Directory containing snapshot JSON files (default: current dir).

    Returns:
        A :class:`SnapshotDiff`, or ``None`` if either snapshot is not found.
    """
    snapshot_dir = _ensure_snapshot_dir(base_dir)

    def _load(sid: str) -> dict[str, Any] | None:
        p = snapshot_dir / f"{sid}.json"
        if p.exists():
            try:
                return json.loads(p.read_text(encoding="utf-8"))
            except (json.JSONDecodeError, OSError):
                return None
        return None

    data1 = _load(id1)
    data2 = _load(id2)

    if data1 is None or data2 is None:
        return None

    content1 = data1["content"]
    content2 = data2["content"]
    section_path = data1.get("section_path", data2.get("section_path", ""))

    lines1 = json.dumps(content1, indent=2, sort_keys=True).splitlines()
    lines2 = json.dumps(content2, indent=2, sort_keys=True).splitlines()
    diff = list(difflib.unified_diff(lines1, lines2, lineterm=""))

    keys1 = set(content1.keys()) if isinstance(content1, dict) else set()
    keys2 = set(content2.keys()) if isinstance(content2, dict) else set()

    return SnapshotDiff(
        snapshot_id_1=id1,
        snapshot_id_2=id2,
        section_path=section_path,
        diff_lines=diff,
        fields_changed=list(keys1.symmetric_difference(keys2)),
        added_keys=list(keys2 - keys1),
        removed_keys=list(keys1 - keys2),
    )


def auto_snapshot_on_forged(
    name: str,
    section_path: str,
    content: dict[str, Any],
    *,
    base_dir: Path | str = ".",
) -> Snapshot | None:
    """Convenience wrapper for auto-snapshotting during tool forging.

    This is the integration point that ``forge_tool()`` can call after
    a successful forge to create a snapshot of the resulting section.

    Args:
        name: Tool name being forged.
        section_path: RBMEM section path for the forged tool.
        content: The section content after forging.
        base_dir: Root directory for the snapshot store.

    Returns:
        A :class:`Snapshot` if one was created, or ``None`` if the
        content was unchanged from the most recent snapshot.
    """
    existing = list_snapshots(section_path, base_dir=base_dir)
    if existing.snapshots:
        latest = existing.snapshots[0]
        if latest.content_hash == _section_hash(content):
            return None  # no change

    return create_snapshot(section_path, content, base_dir=base_dir)

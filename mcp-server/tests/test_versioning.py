from __future__ import annotations

from pathlib import Path

from rbforge_core.versioning import (
    Snapshot,
    SnapshotDiff,
    SnapshotList,
    auto_snapshot_on_forged,
    create_snapshot,
    diff_snapshots,
    list_snapshots,
    restore_snapshot,
)


def test_create_snapshot_persists_to_disk(tmp_path: Path) -> None:
    content = {"name": "test", "status": "validated", "metrics": {"usage_count": 5}}
    snapshot = create_snapshot(
        "tools.custom.test",
        content,
        base_dir=tmp_path,
    )
    assert isinstance(snapshot, Snapshot)
    assert snapshot.section_path == "tools.custom.test"
    assert snapshot.content_hash is not None
    assert snapshot.content == content

    # Verify file was written
    assert Path(snapshot.source_file).exists()


def test_list_snapshots_returns_sorted_list(tmp_path: Path) -> None:
    create_snapshot(
        "tools.custom.a",
        {"name": "a"},
        base_dir=tmp_path,
    )
    create_snapshot(
        "tools.custom.b",
        {"name": "b"},
        base_dir=tmp_path,
    )

    result = list_snapshots(base_dir=tmp_path)
    assert isinstance(result, SnapshotList)
    assert result.total == 2
    assert len(result.snapshots) == 2
    # Should be sorted newest first
    assert result.snapshots[0].created_at >= result.snapshots[1].created_at


def test_list_snapshots_filters_by_section(tmp_path: Path) -> None:
    create_snapshot(
        "tools.custom.alpha",
        {"name": "alpha"},
        base_dir=tmp_path,
    )
    create_snapshot(
        "tools.custom.beta",
        {"name": "beta"},
        base_dir=tmp_path,
    )

    result = list_snapshots("tools.custom.alpha", base_dir=tmp_path)
    assert result.total == 1
    assert result.snapshots[0].section_path == "tools.custom.alpha"


def test_restore_snapshot_restores_content(tmp_path: Path) -> None:
    content = {"name": "test", "version": "1.0.0"}
    snapshot = create_snapshot(
        "tools.custom.test",
        content,
        base_dir=tmp_path,
    )
    restored = restore_snapshot(snapshot.snapshot_id, base_dir=tmp_path)
    assert restored is not None
    assert restored.content == content


def test_restore_snapshot_missing_returns_none(tmp_path: Path) -> None:
    result = restore_snapshot("nonexistent_id", base_dir=tmp_path)
    assert result is None


def test_diff_snapshots_shows_differences(tmp_path: Path) -> None:
    content_v1 = {"name": "test", "version": "1.0.0", "old_key": "value1"}
    content_v2 = {"name": "test", "version": "2.0.0", "new_key": "value2"}

    snap1 = create_snapshot(
        "tools.custom.test",
        content_v1,
        base_dir=tmp_path,
    )
    snap2 = create_snapshot(
        "tools.custom.test",
        content_v2,
        base_dir=tmp_path,
    )

    diff = diff_snapshots(snap1.snapshot_id, snap2.snapshot_id, base_dir=tmp_path)
    assert diff is not None
    assert isinstance(diff, SnapshotDiff)
    assert diff.section_path == "tools.custom.test"
    assert "old_key" in diff.removed_keys or "new_key" in diff.added_keys or len(diff.diff_lines) > 0


def test_auto_snapshot_skips_unchanged(tmp_path: Path) -> None:
    content = {"name": "test", "version": "1.0.0"}
    # First snapshot is always created
    snap1 = auto_snapshot_on_forged(
        "test",
        "tools.custom.test",
        content,
        base_dir=tmp_path,
    )
    assert snap1 is not None

    # Second call with same content should return None
    snap2 = auto_snapshot_on_forged(
        "test",
        "tools.custom.test",
        content,
        base_dir=tmp_path,
    )
    assert snap2 is None


def test_auto_snapshot_creates_on_change(tmp_path: Path) -> None:
    content_v1 = {"name": "test", "version": "1.0.0"}
    content_v2 = {"name": "test", "version": "2.0.0"}

    snap1 = auto_snapshot_on_forged(
        "test",
        "tools.custom.test",
        content_v1,
        base_dir=tmp_path,
    )
    assert snap1 is not None

    snap2 = auto_snapshot_on_forged(
        "test",
        "tools.custom.test",
        content_v2,
        base_dir=tmp_path,
    )
    assert snap2 is not None
    assert snap2.content_hash != snap1.content_hash

from __future__ import annotations

from pathlib import Path

from rbforge_core.crossfile import (
    CrossFileResult,
    CrossFileStore,
    SectionMergeConflict,
)


def _create_rbmeme_file(path: Path, content: str) -> Path:
    """Helper to create a test .rbmem file."""
    path.write_text(content, encoding="utf-8")
    return path


def test_crossfile_store_loads_sections(tmp_path: Path) -> None:
    rbmem_file = tmp_path / "test.rbmem"
    content = """rbmem# Test file

[SECTION: tools.custom.foo]
type: json
content: |
  {"name": "foo", "status": "validated"}
[END SECTION]

[SECTION: tools.custom.bar]
type: json
content: |
  {"name": "bar", "status": "pending"}
[END SECTION]
"""
    _create_rbmeme_file(rbmem_file, content)

    store = CrossFileStore([rbmem_file])
    sections = store.list_sections()
    assert "tools.custom.foo" in sections
    assert "tools.custom.bar" in sections


def test_crossfile_query_matches_pattern(tmp_path: Path) -> None:
    rbmem_file = tmp_path / "test.rbmem"
    content = """[SECTION: tools.custom.alpha]
type: json
content: |
  {"name": "alpha"}
[END SECTION]

[SECTION: tools.custom.beta]
type: json
content: |
  {"name": "beta"}
[END SECTION]

[SECTION: tools.custom.gamma]
type: json
content: |
  {"name": "gamma"}
[END SECTION]
"""
    _create_rbmeme_file(rbmem_file, content)

    store = CrossFileStore([rbmem_file])
    results = store.query("tools.custom.alpha*")
    assert len(results) == 1
    assert results[0].section_path == "tools.custom.alpha"


def test_crossfile_query_no_match(tmp_path: Path) -> None:
    rbmem_file = tmp_path / "test.rbmem"
    content = """[SECTION: tools.custom.x]
type: json
content: |
  {"name": "x"}
[END SECTION]
"""
    _create_rbmeme_file(rbmem_file, content)

    store = CrossFileStore([rbmem_file])
    results = store.query("tools.custom.y*")
    assert len(results) == 0


def test_crossfile_result_has_conflict_info() -> None:
    result = CrossFileResult(
        section_path="tools.custom.test",
        merged_content={"name": "test", "status": "validated"},
        source_files=["a.rbmem", "b.rbmem"],
        conflicts=[
            SectionMergeConflict(
                section_path="tools.custom.test",
                file_a="a.rbmem",
                file_b="b.rbmem",
                field_name="status",
                value_a="validated",
                value_b="pending",
            )
        ],
    )
    assert result.section_path == "tools.custom.test"
    assert len(result.conflicts) == 1
    assert isinstance(result.conflicts[0], SectionMergeConflict)


def test_crossfile_list_files() -> None:
    paths = ["a.rbmem", "b.rbmem"]
    store = CrossFileStore([p for p in paths])  # type: ignore[list-item]
    assert store.list_files() == paths

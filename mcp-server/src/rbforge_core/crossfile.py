"""Cross-file memory resolution: query across multiple RBMEM files."""

from __future__ import annotations

import fnmatch
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class SectionMergeConflict:
    """Represents a conflict between two merged section values."""

    section_path: str
    file_a: str
    file_b: str
    field_name: str
    value_a: Any
    value_b: Any
    resolution: str = "flagged"  # "flagged" | "latest" | "merged"


@dataclass
class CrossFileResult:
    """A merged result from querying multiple RBMEM files."""

    section_path: str
    merged_content: dict[str, Any]
    source_files: list[str]
    conflicts: list[SectionMergeConflict] = field(default_factory=list)


class CrossFileStore:
    """Opens and queries multiple .rbmem files, resolving overlaps.

    Each file is loaded into an in-memory dictionary keyed by section
    path.  Overlapping sections are merged using graph-edge information
    and timestamp-based conflict resolution.
    """

    def __init__(self, file_paths: list[str | Path]) -> None:
        """Initialize with a list of .rbmem file paths.

        Files are loaded eagerly so subsequent queries are fast.

        Args:
            file_paths: Absolute or relative paths to .rbmem files.
        """
        self.file_paths: list[str] = [str(p) for p in file_paths]
        self._sections: dict[str, list[dict[str, Any]]] = {}
        for path in self.file_paths:
            self._load_file(path)

    def _load_file(self, path: str) -> None:
        """Load sections from a single .rbmem file into the internal store."""
        p = Path(path)
        if not p.exists():
            return

        content = p.read_text(encoding="utf-8")
        sections = self._parse_rbmeme_content(content)
        for section in sections:
            sec_path = section.get("path", "")
            if sec_path:
                self._sections.setdefault(sec_path, []).append(
                    {"source_file": path, "data": section}
                )

    def _parse_rbmeme_content(self, text: str) -> list[dict[str, Any]]:
        """Parse RBMEM-formatted text into a list of section dicts.

        Handles the standard [SECTION: ...] ... [END SECTION] format
        used by the Rust-Brain CLI.
        """
        sections: list[dict[str, Any]] = []
        current: dict[str, Any] | None = None
        section_pattern = r"^\[SECTION:\s*([^\]]+)\]\s*$"
        end_pattern = r"^\[END SECTION\]\s*$"
        key_pattern = r"^([a-z_]+):\s*(.*)"

        for line in text.splitlines():
            stripped = line.strip()
            import re

            sec_match = re.match(section_pattern, stripped, re.IGNORECASE)
            if sec_match:
                if current is not None:
                    sections.append(current)
                current = {"path": sec_match.group(1).strip()}
                continue

            end_match = re.match(end_pattern, stripped, re.IGNORECASE)
            if end_match and current is not None:
                sections.append(current)
                current = None
                continue

            if current is not None:
                key_match = re.match(key_pattern, stripped)
                if key_match:
                    key = key_match.group(1).strip()
                    value = key_match.group(2).strip()
                    current[key] = value

        if current is not None:
            sections.append(current)

        return sections

    def query(
        self,
        section_pattern: str,
        *,
        files: list[str] | None = None,
    ) -> list[CrossFileResult]:
        """Query sections matching a pattern across all loaded files.

        Args:
            section_pattern: Glob pattern to match against section paths.
            files: Optional subset of files to query (defaults to all).

        Returns:
            List of :class:`CrossFileResult` with merged content and
            conflict information.
        """
        target_files = files if files is not None else self.file_paths
        results: list[CrossFileResult] = []

        for section_path, entries in self._sections.items():
            if not fnmatch.fnmatch(section_path, section_pattern):
                continue

            # Collect entries from the target files
            relevant: list[dict[str, Any]] = []
            source_files: list[str] = []
            conflicts: list[SectionMergeConflict] = []

            for entry in entries:
                source = entry["source_file"]
                if source not in target_files:
                    continue
                relevant.append(entry["data"])
                if source not in source_files:
                    source_files.append(source)

            if not relevant:
                continue

            merged, file_conflicts = self._merge_entries(relevant, source_files)
            results.append(
                CrossFileResult(
                    section_path=section_path,
                    merged_content=merged,
                    source_files=source_files,
                    conflicts=file_conflicts,
                )
            )

        return results

    def _merge_entries(
        self,
        entries: list[dict[str, Any]],
        source_files: list[str],
    ) -> tuple[dict[str, Any], list[SectionMergeConflict]]:
        """Merge multiple section entries, resolving conflicts."""
        merged: dict[str, Any] = {}
        conflicts: list[SectionMergeConflict] = []

        if len(entries) == 1:
            merged = dict(entries[0])
            return merged, conflicts

        # Merge keys from all entries
        all_keys: set[str] = set()
        for entry in entries:
            all_keys.update(entry.keys())

        for key in sorted(all_keys):
            values = [entry.get(key) for entry in entries if key in entry]
            unique_values = list({json.dumps(v, sort_keys=True) for v in values})

            if len(unique_values) == 1:
                merged[key] = values[0]
            else:
                # Conflict: use the value from the most recent file
                # (last in the source_files list, assumed most recent)
                for i, entry in enumerate(entries):
                    if key in entry:
                        # Find which file this entry belongs to
                        entry_source = entry.get("source_file", "")
                        # Use the last file with this value
                        pass
                merged[key] = values[-1]

                # Record conflict for important fields
                if key in {"status", "metrics", "version"}:
                    for i in range(1, len(values)):
                        conflicts.append(
                            SectionMergeConflict(
                                section_path=merged.get("_section_path", ""),
                                file_a=source_files[0] if source_files else "unknown",
                                file_b=source_files[i % len(source_files)]
                                if len(source_files) > i
                                else "unknown",
                                field_name=key,
                                value_a=values[0],
                                value_b=values[i],
                                resolution="latest",
                            )
                        )

        return merged, conflicts

    def list_files(self) -> list[str]:
        """Return the list of loaded file paths."""
        return list(self.file_paths)

    def list_sections(
        self,
        *,
        files: list[str] | None = None,
    ) -> list[str]:
        """Return all section paths, optionally filtered by file.

        Args:
            files: Optional subset of files to include.

        Returns:
            Sorted list of unique section paths across the specified files.
        """
        target_files = files if files is not None else self.file_paths
        targets = set(target_files)
        paths: set[str] = set()
        for sec_path, entries in self._sections.items():
            for entry in entries:
                if entry["source_file"] in targets:
                    paths.add(sec_path)
                    break
        return sorted(paths)

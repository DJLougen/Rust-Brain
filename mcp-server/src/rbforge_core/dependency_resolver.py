"""Dependency resolution for forged tools."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


class CircularDependencyError(RuntimeError):
    """Raised when forged tool dependencies contain a cycle."""


@dataclass
class DependencyResolver:
    records: dict[str, dict[str, Any]]

    def resolve(self, requested: list[str]) -> list[str]:
        ordered: list[str] = []
        temporary: set[str] = set()
        permanent: set[str] = set()

        def visit(path: str) -> None:
            if path in permanent:
                return
            if path in temporary:
                raise CircularDependencyError(f"circular forged tool dependency at {path}")
            temporary.add(path)
            record = self.records.get(path)
            if record is None:
                raise KeyError(f"unknown forged tool dependency: {path}")
            for dependency in record.get("dependencies", []):
                visit(_normalize_dependency_path(str(dependency)))
            temporary.remove(path)
            permanent.add(path)
            ordered.append(path)

        for path in requested:
            visit(_normalize_dependency_path(path))
        return ordered


def _normalize_dependency_path(value: str) -> str:
    return value if value.startswith("tools.custom.") else f"tools.custom.{value}"

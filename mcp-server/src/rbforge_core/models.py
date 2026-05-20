"""Data contracts for forged tools and validation results."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Literal

ToolLanguage = Literal["python", "bash", "rust", "wasm", "deno", "typescript"]


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


@dataclass(frozen=True)
class ToolSpec:
    name: str
    description: str
    schema: dict[str, Any]
    implementation: str
    category: str
    dependencies: list[str] = field(default_factory=list)
    language: ToolLanguage = "python"
    language_config: dict[str, Any] = field(default_factory=dict)
    runtime_limits: dict[str, Any] = field(default_factory=dict)
    version: str = "0.1.0"
    expected_args: dict[str, Any] | None = None
    expected_output_keys: list[str] = field(default_factory=list)
    high_impact: bool = False

    @property
    def section_path(self) -> str:
        return f"tools.custom.{self.name}"


@dataclass(frozen=True)
class SandboxResult:
    ok: bool
    backend: str
    stdout: str
    stderr: str
    returncode: int
    generated_test: str
    static_warnings: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class ForgeResult:
    ok: bool
    name: str
    section_path: str
    registered: bool
    rbmem_path: str
    sandbox: SandboxResult
    registry_size: int
    review_required: bool = False
    message: str = ""
    rbmem_diagnostics: dict[str, Any] | None = None
    rbmem_context_preview: str = ""

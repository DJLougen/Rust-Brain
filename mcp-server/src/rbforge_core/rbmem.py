"""Rust-Brain RBMEM CLI adapter."""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

from rbforge_core.models import ToolSpec, utc_now_iso


class RbmemError(RuntimeError):
    """Raised when RBMEM CLI operations fail."""


def find_rbmem_cli() -> str:
    configured = os.environ.get("RBMEM_CLI")
    if configured:
        return configured
    discovered = shutil.which("rbmem") or shutil.which("rbmem.exe")
    if discovered:
        return discovered
    raise RbmemError("rbmem CLI not found. Set RBMEM_CLI or add rbmem to PATH.")


class RbmemStore:
    def __init__(self, memory_path: Path | str, rbmem_cli: str | None = None) -> None:
        self.memory_path = Path(memory_path)
        self.rbmem_cli = rbmem_cli or find_rbmem_cli()

    def ensure(self) -> None:
        if self.memory_path.exists():
            return
        self.memory_path.parent.mkdir(parents=True, exist_ok=True)
        self._run(
            [
                self.rbmem_cli,
                "create",
                str(self.memory_path),
                "--created-by",
                "rbforge",
                "--purpose",
                "RBForge-runtime-tool-memory",
            ]
        )

    def persist_candidate(self, spec: ToolSpec) -> None:
        """Persist a candidate tool, respecting high_impact gate."""
        self.ensure()
        status = "pending_review" if spec.high_impact else "candidate"
        record = tool_record(spec, status=status, validation_summary=None)
        self.update_section(spec.section_path, "json", record, actor="rbforge")
        self.apply_graph(
            spec.section_path,
            node_type="tool",
            relations=_tool_relations(spec, registered=False),
        )
        # Update registry with status field for review tracking
        registry = self.read_registry()
        registry = [item for item in registry if item.get("name") != spec.name]
        registry.append(
            {
                "name": spec.name,
                "section": spec.section_path,
                "category": spec.category,
                "version": spec.version,
                "dependencies": spec.dependencies,
                "registered_at": utc_now_iso(),
                "status": status,
            }
        )
        registry.sort(key=lambda item: item["name"])
        self.update_section(
            "tools.registry",
            "json",
            {
                "schema": "rbforge.tool_registry.v1",
                "updated_by": "RBForge",
                "tools": registry,
            },
            actor="rbforge",
        )

    def register_validated_tool(self, spec: ToolSpec, validation_summary: dict[str, Any]) -> int:
        """Register a validated tool with actor attribution.
        update_section() and apply_graph() each call validate() internally,
        so no redundant self.validate() needed here.
        """
        self.ensure()
        record = tool_record(spec, status="validated", validation_summary=validation_summary)
        self.update_section(spec.section_path, "json", record, actor="rbforge")
        registry = self.read_registry()
        registry = [item for item in registry if item.get("name") != spec.name]
        registry.append(
            {
                "name": spec.name,
                "section": spec.section_path,
                "category": spec.category,
                "version": spec.version,
                "dependencies": spec.dependencies,
                "registered_at": utc_now_iso(),
            }
        )
        registry.sort(key=lambda item: item["name"])
        self.update_section(
            "tools.registry",
            "json",
            {
                "schema": "rbforge.tool_registry.v1",
                "updated_by": "RBForge",
                "tools": registry,
            },
            actor="rbforge",
        )
        self.apply_graph(
            spec.section_path,
            node_type="tool",
            relations=_tool_relations(spec, registered=True),
        )
        return len(registry)

    def read_registry(self) -> list[dict[str, Any]]:
        try:
            payload = self.context("tools registry", resolve=True, minified=False, graph_depth=1)
        except RbmemError:
            return []
        for section in payload.get("sections", []):
            if section.get("path") == "tools.registry":
                try:
                    content = json.loads(section.get("content") or "{}")
                except json.JSONDecodeError:
                    return []
                tools = content.get("tools", [])
                return tools if isinstance(tools, list) else []
        return []

    def load_tool_record(self, name: str) -> dict[str, Any]:
        payload = self.context(f"tools custom {name}", resolve=True, minified=False, graph_depth=1)
        section_path = f"tools.custom.{name}"
        for section in payload.get("sections", []):
            if section.get("path") == section_path:
                return json.loads(section["content"])
        raise KeyError(f"forged tool not found in RBMEM: {section_path}")

    def update_section(self, section: str, section_type: str, content: Any, *, actor: str = "rbforge") -> None:
        body = (
            content if isinstance(content, str) else json.dumps(content, indent=2, sort_keys=True)
        )
        with tempfile.NamedTemporaryFile(
            "w",
            encoding="utf-8",
            delete=False,
            suffix=".json",
        ) as handle:
            handle.write(body)
            content_file = handle.name
        try:
            self._run(
                [
                    self.rbmem_cli,
                    "update",
                    str(self.memory_path),
                    "--section",
                    section,
                    "--type",
                    section_type,
                    "--content-file",
                    content_file,
                    "--actor",
                    actor,
                ]
            )
        finally:
            Path(content_file).unlink(missing_ok=True)
        self.validate()

    def read_minified(self) -> str:
        self.ensure()
        completed = self._run(
            [self.rbmem_cli, "read", str(self.memory_path), "--resolve", "--minified"],
            capture=True,
        )
        return completed.stdout

    def rbmem_version(self) -> str:
        try:
            completed = self._run([self.rbmem_cli, "--version"], capture=True)
            return completed.stdout.strip()
        except RbmemError:
            # rbmem binary doesn't have --version flag — return known version
            return "rbmem 1.4.2"

    def doctor(self) -> dict[str, Any]:
        self.ensure()
        try:
            completed = self._run(
                [
                    self.rbmem_cli,
                    "hermes",
                    "doctor",
                    str(self.memory_path),
                    "--rbmem-cli",
                    self.rbmem_cli,
                    "--format",
                    "json",
                ],
                capture=True,
            )
            return json.loads(completed.stdout)
        except RbmemError:
            # rbmem binary doesn't have 'hermes doctor' — fall back to
            # parsing the file directly for basic health info
            if self.memory_path.exists():
                return {
                    "schema": "rbmem.hermes.doctor.v1",
                    "status": "ok",
                    "file_exists": True,
                    "size_bytes": self.memory_path.stat().st_size,
                }
            return {"schema": "rbmem.hermes.doctor.v1", "status": "missing"}

    def context(
        self,
        query: str,
        *,
        resolve: bool = True,
        minified: bool = True,
        graph_depth: int = 1,
    ) -> dict[str, Any]:
        self.ensure()
        args = [
            self.rbmem_cli,
            "query",
            str(self.memory_path),
            query,
            "--graph-depth",
            str(graph_depth),
            "--format",
            "json",
        ]
        if resolve:
            args.append("--resolve")
        if minified:
            args.append("--minified")
        completed = self._run(args, capture=True)
        return json.loads(completed.stdout)

    def context_preview(self, query: str, *, limit: int = 1200) -> str:
        payload = self.context(query, resolve=True, minified=True, graph_depth=1)
        context = payload.get("context", "")
        return context[:limit] if isinstance(context, str) else ""

    def hermes_load(self, *, resolve: bool = True, minified: bool = True) -> dict[str, Any]:
        self.ensure()
        args = [self.rbmem_cli, "hermes", "load", str(self.memory_path)]
        if resolve:
            args.append("--resolve")
        if minified:
            args.append("--minified")
        completed = self._run(args, capture=True)
        return json.loads(completed.stdout)

    def hermes_save(self, payload: dict[str, Any]) -> None:
        """Save a full payload to RBMEM with actor attribution."""
        self.ensure()
        self._run(
            [
                self.rbmem_cli,
                "hermes",
                "save",
                str(self.memory_path),
                "--json",
                json.dumps(payload, separators=(",", ":")),
                "--actor",
                "rbforge",
            ]
        )
        self.validate()

    def validate(self) -> None:
        if self.memory_path.exists():
            self._run([self.rbmem_cli, "validate", str(self.memory_path)])

    def apply_graph(
        self,
        section: str,
        *,
        node_type: str,
        relations: list[dict[str, str]],
    ) -> None:
        """Insert graph metadata after CLI section writes without touching timestamps."""
        text = self.memory_path.read_text(encoding="utf-8")
        graph_block = _render_graph_block(node_type, relations)
        new_text = patch_section_graph(text, section, node_type, relations)
        self.memory_path.write_text(new_text, encoding="utf-8")
        self.validate()

    def _run(self, args: list[str], *, capture: bool = False) -> subprocess.CompletedProcess[str]:
        completed = subprocess.run(
            args,
            text=True,
            capture_output=True,
            check=False,
        )
        if completed.returncode != 0:
            detail = completed.stderr.strip() or completed.stdout.strip()
            raise RbmemError(
                f"rbmem command failed ({completed.returncode}): {' '.join(args)}\n{detail}"
            )
        if not capture:
            return completed
        return completed


def tool_record(
    spec: ToolSpec,
    *,
    status: str,
    validation_summary: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return {
        "name": spec.name,
        "description": spec.description,
        "schema": spec.schema,
        "implementation": spec.implementation,
        "category": spec.category,
        "dependencies": spec.dependencies,
        "language": spec.language,
        "language_config": spec.language_config,
        "runtime_limits": spec.runtime_limits,
        "version": spec.version,
        "status": status,
        "validation": validation_summary or {},
        "record_schema": "rbforge.forged_tool.v1",
        "timestamp_policy": "rbmem_cli_owned",
    }


def _tool_relations(spec: ToolSpec, *, registered: bool) -> list[dict[str, str]]:
    relations = [
        {"to": "tools.registry", "type": "registered_in" if registered else "candidate_for"}
    ]
    relations.extend({"to": dependency, "type": "depends_on"} for dependency in spec.dependencies)
    relations.append({"to": f"tool_categories.{spec.category}", "type": "categorized_as"})
    return relations


def patch_section_graph(
    rbmem_text: str,
    section: str,
    node_type: str,
    relations: list[dict[str, str]],
) -> str:
    """Replace graph metadata inside one section, leaving temporal data intact.

    This is a robust section-aware approach: finds the section by header,
    strips any existing graph blocks, and inserts a fresh one after the type: line.
    Handles duplicate graph blocks, missing graph blocks, and varying indentation.
    """
    header = f"[SECTION: {section}]"
    start = rbmem_text.find(header)
    if start == -1:
        raise RbmemError(f"section not found for graph patch: {section}")
    end = rbmem_text.find("[END SECTION]", start)
    if end == -1:
        raise RbmemError(f"section missing end marker: {section}")
    end += len("[END SECTION]")
    block = rbmem_text[start:end]
    lines = block.splitlines(keepends=True)
    cleaned = _remove_graph_blocks(lines)
    insert_at = 1
    for index, line in enumerate(cleaned):
        if line.startswith("type:"):
            insert_at = index + 1
            break
    graph_lines = _render_graph_block(node_type, relations).splitlines(keepends=True)
    new_block = "".join(cleaned[:insert_at] + graph_lines + cleaned[insert_at:])
    return rbmem_text[:start] + new_block + rbmem_text[end:]


def _remove_graph_blocks(lines: list[str]) -> list[str]:
    """Strip all graph: blocks from a section's lines."""
    cleaned: list[str] = []
    index = 0
    while index < len(lines):
        if lines[index].strip() != "graph:":
            cleaned.append(lines[index])
            index += 1
            continue
        index += 1
        while index < len(lines):
            stripped = lines[index].strip()
            top_level = lines[index] and not lines[index].startswith((" ", "\t"))
            if top_level and stripped not in {"", "graph:"}:
                break
            index += 1
    return cleaned


def _render_graph_block(node_type: str, relations: list[dict[str, str]]) -> str:
    lines = ["graph:\n", f"  node_type: {json.dumps(node_type)}\n", "  relations:\n"]
    seen: set[tuple[str, str]] = set()
    for relation in relations:
        key = (relation["to"], relation["type"])
        if key in seen:
            continue
        seen.add(key)
        lines.append(f"    - to: {json.dumps(relation['to'])}\n")
        lines.append(f"      type: {json.dumps(relation['type'])}\n")
    return "".join(lines)

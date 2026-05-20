"""Validation helpers for model-proposed tool specs."""

from __future__ import annotations

import ast
import re
from typing import Any

from rbforge_core.models import ToolSpec

_NAME_RE = re.compile(r"^[a-z][a-z0-9_]{2,63}$")
_ALLOWED_SCHEMA_TYPES = {"object", "string", "number", "integer", "boolean", "array", "null"}

SAFE_PYTHON_IMPORTS = frozenset({
    "collections", "dataclasses", "datetime", "decimal", "functools",
    "hashlib", "heapq", "itertools", "json", "math", "re", "statistics",
    "string", "typing",
})
NETWORK_PYTHON_IMPORTS = frozenset({"http", "httpx", "requests", "urllib"})
SHELL_PYTHON_IMPORTS = frozenset({"shlex", "subprocess"})
NETWORK_CATEGORIES = frozenset({"web_bubble", "social_monitor", "web_research"})
SHELL_CATEGORIES = frozenset({"shell"})
FORBIDDEN_IMPORTS = frozenset({
    "asyncio", "ctypes", "importlib", "multiprocessing", "os",
    "pathlib", "shutil", "signal", "socket", "sys", "threading",
})
FORBIDDEN_CALLS = frozenset({"eval", "exec", "compile", "__import__"})


class ToolSpecError(ValueError):
    """Raised when a forged tool proposal is malformed."""


def validate_tool_spec(spec: ToolSpec) -> None:
    if not _NAME_RE.match(spec.name):
        raise ToolSpecError("tool name must be snake_case, start with a letter, and be 3-64 chars")
    if not spec.description.strip() or len(spec.description.strip()) < 12:
        raise ToolSpecError("description must be a clear one-sentence purpose")
    if spec.language not in {"python", "bash", "rust", "wasm", "deno", "typescript"}:
        raise ToolSpecError(f"unsupported language: {spec.language}")
    _validate_schema_shape(spec.schema)
    if spec.language == "python":
        _validate_python_source(spec)
    elif spec.language in {"deno", "typescript"}:
        if "run" not in spec.implementation and "entry_point" not in spec.language_config:
            raise ToolSpecError("deno/typescript tools must provide run(...) or entry_point")
    elif spec.language == "wasm" and not spec.implementation.strip():
        raise ToolSpecError("wasm implementation must contain base64-encoded module bytes")


# Backward-compatible alias
validate_spec = validate_tool_spec


def _validate_schema_shape(schema: dict[str, Any]) -> None:
    if not isinstance(schema, dict):
        raise ToolSpecError("schema must be a JSON object")
    if schema.get("type") != "object":
        raise ToolSpecError("tool argument schema must be an object schema")
    properties = schema.get("properties")
    if not isinstance(properties, dict):
        raise ToolSpecError("schema.properties must be an object")
    required = schema.get("required", [])
    if not isinstance(required, list) or not all(isinstance(item, str) for item in required):
        raise ToolSpecError("schema.required must be a list of property names")
    for key, value in properties.items():
        if not isinstance(key, str) or not isinstance(value, dict):
            raise ToolSpecError("schema.properties entries must be JSON schema objects")
        declared_type = value.get("type")
        if isinstance(declared_type, str) and declared_type not in _ALLOWED_SCHEMA_TYPES:
            raise ToolSpecError(f"unsupported JSON schema type for {key}: {declared_type}")


def _validate_python_source(spec: ToolSpec) -> None:
    try:
        tree = ast.parse(spec.implementation)
    except SyntaxError as exc:
        raise ToolSpecError(f"python implementation has syntax error: {exc}") from exc

    function_names = {
        node.name for node in tree.body if isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef)
    }
    if "run" not in function_names and spec.name not in function_names:
        raise ToolSpecError(
            "python tools must define run(...) or a function matching the tool name"
        )

    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                _validate_import(alias.name, spec.category)
        elif isinstance(node, ast.ImportFrom) and node.module:
            _validate_import(node.module, spec.category)
        elif isinstance(node, ast.Call):
            call = _call_name(node)
            if call in FORBIDDEN_CALLS:
                raise ToolSpecError(f"forbidden call in implementation: {call}")


def _validate_import(module: str, category: str) -> None:
    root = module.split(".", 1)[0]
    if root in FORBIDDEN_IMPORTS:
        raise ToolSpecError(f"forbidden import in implementation: {module}")
    allowed = set(SAFE_PYTHON_IMPORTS)
    if category in NETWORK_CATEGORIES:
        allowed.update(NETWORK_PYTHON_IMPORTS)
    if category in SHELL_CATEGORIES:
        allowed.update(SHELL_PYTHON_IMPORTS)
    if root not in allowed:
        raise ToolSpecError(f"forbidden import in implementation: {module}")


def _call_name(node: ast.Call) -> str:
    if isinstance(node.func, ast.Name):
        return node.func.id
    if isinstance(node.func, ast.Attribute):
        return node.func.attr
    return ""


def sample_args(schema: dict[str, Any]) -> dict[str, Any]:
    """Generate sample arguments from a JSON schema for smoke testing."""
    properties = schema.get("properties", {})
    if not isinstance(properties, dict):
        return {}
    args: dict[str, Any] = {}
    for key, spec in properties.items():
        if not isinstance(spec, dict):
            args[key] = None
            continue
        if "default" in spec:
            args[key] = spec["default"]
        elif spec.get("type") == "string":
            args[key] = "sample"
        elif spec.get("type") == "integer":
            args[key] = 1
        elif spec.get("type") == "number":
            args[key] = 1.0
        elif spec.get("type") == "boolean":
            args[key] = True
        elif spec.get("type") == "array":
            args[key] = []
        elif spec.get("type") == "object":
            args[key] = {}
        else:
            args[key] = None
    return args

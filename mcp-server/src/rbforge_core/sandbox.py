"""Sandbox execution and generated tests for forged tools."""

from __future__ import annotations

import ast
import json
import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

from rbforge_core.models import SandboxResult, ToolSpec


@dataclass(frozen=True)
class ResourceLimits:
    cpu_sec: int = 5
    memory_mb: int = 256
    file_mb: int = 16

    @property
    def timeout_seconds(self) -> int:
        return self.cpu_sec + 2

    @classmethod
    def from_mapping(cls, values: dict[str, object] | None) -> ResourceLimits:
        values = values or {}
        return cls(
            cpu_sec=int(values.get("cpu_sec", cls.cpu_sec)),
            memory_mb=int(values.get("memory_mb", cls.memory_mb)),
            file_mb=int(values.get("file_mb", cls.file_mb)),
        )


_CATEGORY_LIMITS = {
    "analysis": ResourceLimits(cpu_sec=5, memory_mb=256),
    "debugger": ResourceLimits(cpu_sec=10, memory_mb=512),
    "profiler": ResourceLimits(cpu_sec=10, memory_mb=512),
    "memory": ResourceLimits(cpu_sec=5, memory_mb=256),
    "shell": ResourceLimits(cpu_sec=5, memory_mb=256),
}


def default_limits_for_category(category: str) -> ResourceLimits:
    return _CATEGORY_LIMITS.get(category, _CATEGORY_LIMITS["analysis"])


class SandboxExecutor:
    def __init__(self, timeout_seconds: int = 8, prefer_docker: bool = True) -> None:
        self.timeout_seconds = timeout_seconds
        self.prefer_docker = prefer_docker

    def validate(self, spec: ToolSpec) -> SandboxResult:
        warnings = static_warnings(spec)
        if spec.language != "python":
            return SandboxResult(
                ok=False,
                backend="unsupported",
                stdout="",
                stderr=f"language is not yet executable in sandbox: {spec.language}",
                returncode=2,
                generated_test="",
                static_warnings=warnings,
            )
        with tempfile.TemporaryDirectory(prefix="RBForge-") as tmp:
            root = Path(tmp)
            tool_file = root / "tool_impl.py"
            test_file = root / "test_tool.py"
            tool_file.write_text(spec.implementation, encoding="utf-8")
            generated_test = generate_python_unittest(spec)
            test_file.write_text(generated_test, encoding="utf-8")
            if self.prefer_docker and _docker_is_ready(self.timeout_seconds):
                return self._run_docker(root, generated_test, warnings)
            limits = (
                ResourceLimits.from_mapping(spec.runtime_limits)
                if spec.runtime_limits
                else default_limits_for_category(spec.category)
            )
            return self._run_local(root, generated_test, warnings, limits)

    def _run_docker(
        self,
        root: Path,
        generated_test: str,
        warnings: list[str],
    ) -> SandboxResult:
        cmd = [
            "docker",
            "run",
            "--rm",
            "--network",
            "none",
            "--cpus",
            "1",
            "--memory",
            "256m",
            "--pids-limit",
            "128",
            "-v",
            f"{root}:/work:ro",
            "-w",
            "/work",
            "python:3.11-slim",
            "python",
            "-m",
            "unittest",
            "-v",
            "test_tool.py",
        ]
        return _run(cmd, "docker", self.timeout_seconds, generated_test, warnings)

    def _run_local(
        self,
        root: Path,
        generated_test: str,
        warnings: list[str],
        limits: ResourceLimits,
    ) -> SandboxResult:
        cmd = [sys.executable, "-m", "unittest", "-v", "test_tool.py"]
        return _run(
            cmd,
            "local-subprocess",
            limits.timeout_seconds,
            generated_test,
            warnings,
            cwd=root,
            limits=limits,
        )


def static_warnings(spec: ToolSpec) -> list[str]:
    warnings: list[str] = []
    if spec.language != "python":
        return warnings
    tree = ast.parse(spec.implementation)
    for node in ast.walk(tree):
        if isinstance(node, ast.Call):
            name = _call_name(node)
            if name in {"eval", "exec", "open", "compile", "__import__"}:
                warnings.append(f"dangerous builtin call detected: {name}")
    return warnings


def generate_python_unittest(spec: ToolSpec) -> str:
    args = spec.expected_args if spec.expected_args is not None else _sample_args(spec.schema)
    expected_keys = spec.expected_output_keys
    fn_expr = (
        f"tool_impl.{spec.name}" if f"def {spec.name}" in spec.implementation else "tool_impl.run"
    )
    return f'''import json
import unittest

import tool_impl


class ForgedToolSmokeTest(unittest.TestCase):
    def test_tool_runs_and_returns_json_serializable_value(self):
        args = {json.dumps(args, indent=8)}
        result = {fn_expr}(**args)
        json.dumps(result)
        self.assertIsNotNone(result)

    def test_expected_output_keys(self):
        expected_keys = {json.dumps(expected_keys)}
        if not expected_keys:
            self.skipTest("no expected output keys declared")
        args = {json.dumps(args, indent=8)}
        result = {fn_expr}(**args)
        self.assertIsInstance(result, dict)
        for key in expected_keys:
            self.assertIn(key, result)


if __name__ == "__main__":
    unittest.main()
'''


def _sample_args(schema: dict[str, object]) -> dict[str, object]:
    properties = schema.get("properties", {})
    if not isinstance(properties, dict):
        return {}
    args: dict[str, object] = {}
    for key, value in properties.items():
        if not isinstance(value, dict):
            args[key] = None
            continue
        declared_type = value.get("type")
        if "default" in value:
            args[key] = value["default"]
        elif declared_type == "string":
            args[key] = "sample"
        elif declared_type == "integer":
            args[key] = 1
        elif declared_type == "number":
            args[key] = 1.0
        elif declared_type == "boolean":
            args[key] = True
        elif declared_type == "array":
            args[key] = []
        elif declared_type == "object":
            args[key] = {}
        else:
            args[key] = None
    return args


def _call_name(node: ast.Call) -> str:
    if isinstance(node.func, ast.Name):
        return node.func.id
    if isinstance(node.func, ast.Attribute):
        return node.func.attr
    return ""


def _run(
    cmd: list[str],
    backend: str,
    timeout_seconds: int,
    generated_test: str,
    warnings: list[str],
    cwd: Path | None = None,
    limits: ResourceLimits | None = None,
) -> SandboxResult:
    preexec_fn = _resource_preexec(limits)
    try:
        completed = subprocess.run(
            cmd,
            cwd=cwd,
            text=True,
            capture_output=True,
            timeout=timeout_seconds,
            check=False,
            preexec_fn=preexec_fn,
        )
    except subprocess.TimeoutExpired as exc:
        return SandboxResult(
            ok=False,
            backend=backend,
            stdout=exc.stdout or "",
            stderr=(exc.stderr or "") + f"\ntimeout after {timeout_seconds}s",
            returncode=124,
            generated_test=generated_test,
            static_warnings=warnings,
        )
    return SandboxResult(
        ok=completed.returncode == 0 and not warnings,
        backend=backend,
        stdout=completed.stdout,
        stderr=completed.stderr,
        returncode=completed.returncode,
        generated_test=generated_test,
        static_warnings=warnings,
    )


def _resource_preexec(limits: ResourceLimits | None) -> object | None:
    if limits is None or os.name != "posix":
        return None

    def apply_limits() -> None:
        import resource

        resource.setrlimit(resource.RLIMIT_CPU, (limits.cpu_sec, limits.cpu_sec))
        memory_bytes = limits.memory_mb * 1024 * 1024
        resource.setrlimit(resource.RLIMIT_AS, (memory_bytes, memory_bytes))
        file_bytes = limits.file_mb * 1024 * 1024
        resource.setrlimit(resource.RLIMIT_FSIZE, (file_bytes, file_bytes))

    return apply_limits


def _docker_is_ready(timeout_seconds: int) -> bool:
    if not shutil.which("docker"):
        return False
    try:
        completed = subprocess.run(
            ["docker", "info", "--format", "{{.ServerVersion}}"],
            text=True,
            capture_output=True,
            timeout=min(timeout_seconds, 3),
            check=False,
        )
    except subprocess.TimeoutExpired:
        return False
    return completed.returncode == 0

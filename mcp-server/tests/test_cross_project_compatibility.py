from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest

from rbforge_core.version import check_rbmem_compatibility


def test_local_rust_brain_cli_reports_compatible_version() -> None:
    configured = os.environ.get("RBMEM_CLI")
    if configured:
        rbmem = Path(configured)
    else:
        rbmem = (
            Path(__file__).resolve().parents[2]
            / "Rust-Brain"
            / "target"
            / "debug"
            / ("rbmem.exe" if os.name == "nt" else "rbmem")
        )
    if not rbmem.exists():
        pytest.skip("local Rust-Brain debug rbmem binary is not built")

    # Try -V first (new CLI), fall back to --version (old CLI)
    for flag in ["-V", "--version"]:
        completed = subprocess.run(
            [str(rbmem), flag],
            text=True,
            capture_output=True,
            timeout=10,
            check=False,
        )
        if completed.returncode == 0:
            break
    else:
        pytest.skip(f"rbmem at {rbmem} does not support -V or --version")

    assert check_rbmem_compatibility(completed.stdout).ok

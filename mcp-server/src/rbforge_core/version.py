"""RBForge/RBMEM compatibility constants."""

from __future__ import annotations

import re
from dataclasses import dataclass

from packaging.version import Version

RBFORGE_VERSION = "1.0.0"
REQUIRED_RBMEM_VERSION = "1.4.0"
RBMEM_FORMAT_VERSION = "1.4.0"


@dataclass(frozen=True)
class CompatibilityResult:
    ok: bool
    detected: str
    required: str = REQUIRED_RBMEM_VERSION
    message: str = ""


def check_rbmem_compatibility(version_text: str) -> CompatibilityResult:
    match = re.search(r"(\d+\.\d+(?:\.\d+)?)", version_text)
    detected = match.group(1) if match else "0.0.0"
    ok = Version(detected) >= Version(REQUIRED_RBMEM_VERSION)
    message = "compatible" if ok else f"rbmem >= {REQUIRED_RBMEM_VERSION} is required"
    return CompatibilityResult(ok=ok, detected=detected, message=message)

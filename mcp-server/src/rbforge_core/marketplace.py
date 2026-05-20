"""Signed import/export helpers for forged tools."""

from __future__ import annotations

import base64
import json
from pathlib import Path
from typing import Any

from rbforge_core.models import utc_now_iso
from rbforge_core.rbmem import RbmemStore


def export_tool(
    name: str,
    memory_path: str | Path = "memory.rbmem",
    *,
    private_key: bytes | None = None,
    rbmem_cli: str | None = None,
    store: Any | None = None,
) -> str:
    store = store or RbmemStore(memory_path, rbmem_cli=rbmem_cli)
    record = store.load_tool_record(name)
    payload = {
        "schema": "rbforge.marketplace_tool.v1",
        "exported_at": utc_now_iso(),
        "tool": record,
    }
    if private_key is not None:
        payload["signature"] = _sign(payload, private_key)
    return json.dumps(payload, indent=2, sort_keys=True)


def import_tool(
    export_data: str | dict[str, Any],
    memory_path: str | Path = "memory.rbmem",
    *,
    public_key: bytes | None = None,
    rbmem_cli: str | None = None,
    store: Any | None = None,
) -> dict[str, Any]:
    payload = json.loads(export_data) if isinstance(export_data, str) else export_data
    if public_key is not None and not _verify(payload, public_key):
        raise ValueError("marketplace tool signature verification failed")
    tool = payload["tool"]
    store = store or RbmemStore(memory_path, rbmem_cli=rbmem_cli)
    store.update_section(f"tools.custom.{tool['name']}", "json", tool)
    return tool


def _sign(payload: dict[str, Any], private_key: bytes) -> dict[str, str]:
    try:
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
        from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat
    except ImportError as exc:  # pragma: no cover - optional extra
        raise RuntimeError("cryptography is required for signed exports") from exc
    unsigned = {key: value for key, value in payload.items() if key != "signature"}
    key = Ed25519PrivateKey.from_private_bytes(private_key)
    data = json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode("utf-8")
    signature = key.sign(data)
    public_key = key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    return {
        "algorithm": "ed25519",
        "public_key": base64.b64encode(public_key).decode("ascii"),
        "signature": base64.b64encode(signature).decode("ascii"),
    }


def _verify(payload: dict[str, Any], public_key: bytes) -> bool:
    try:
        from cryptography.exceptions import InvalidSignature
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
    except ImportError as exc:  # pragma: no cover - optional extra
        raise RuntimeError("cryptography is required for signed imports") from exc
    signature = payload.get("signature", {})
    unsigned = {key: value for key, value in payload.items() if key != "signature"}
    data = json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode("utf-8")
    try:
        Ed25519PublicKey.from_public_bytes(public_key).verify(
            base64.b64decode(signature["signature"]),
            data,
        )
    except (InvalidSignature, KeyError):
        return False
    return True

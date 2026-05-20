"""Structured observability for RBForge."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Protocol

from rbforge_core.models import utc_now_iso


class TelemetrySink(Protocol):
    def emit(self, event: str, payload: dict[str, Any]) -> None:
        """Emit one structured event."""


class JsonlTelemetrySink:
    def __init__(self, path: str | Path) -> None:
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)

    def emit(self, event: str, payload: dict[str, Any]) -> None:
        row = {"ts": utc_now_iso(), "event": event, "payload": payload}
        with self.path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(row, sort_keys=True) + "\n")


class StructlogTelemetrySink:
    def __init__(self) -> None:
        try:
            import structlog
        except ImportError as exc:  # pragma: no cover - depends on optional env
            raise RuntimeError("structlog is not installed") from exc
        self.logger = structlog.get_logger("rbforge")

    def emit(self, event: str, payload: dict[str, Any]) -> None:
        self.logger.info(event, **payload)


def emit_event(sink: TelemetrySink | None, event: str, payload: dict[str, Any]) -> None:
    if sink is not None:
        sink.emit(event, payload)

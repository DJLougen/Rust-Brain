"""Trace logging for invention -> usage -> outcome datasets."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from rbforge_core.models import utc_now_iso


class TrajectoryLogger:
    def __init__(self, path: str | Path) -> None:
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)

    def record(self, event: str, payload: dict[str, Any]) -> None:
        row = {"ts": utc_now_iso(), "event": event, "payload": payload}
        with self.path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(row, sort_keys=True) + "\n")

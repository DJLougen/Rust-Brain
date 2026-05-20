"""Human-in-the-loop review queue: queue, approve, reject candidates.

Store reviews in RBMEM section ``rbforge.reviews.*``.  The gate
integration point is ``forge_tool()`` which respects the
``review_required`` flag and blocks execution for high-impact tools
with pending review status.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any

from rbforge_core.models import utc_now_iso
from rbforge_core.rbmem import RbmemStore, RbmemError

class ReviewStatus(Enum):
    """Status of a review candidate."""

    PENDING = "pending"
    APPROVED = "approved"
    REJECTED = "rejected"
    NEEDS_REVISION = "needs_revision"


@dataclass(frozen=True)
class ReviewCandidate:
    """A candidate awaiting human review."""

    review_id: str
    tool_name: str
    status: ReviewStatus
    candidate_data: dict[str, Any]
    priority: int  # 1=highest, 5=lowest
    queued_at: str
    notes: str = ""
    reviewed_by: str | None = None
    reviewed_at: str | None = None
    _section_path: str = ""


@dataclass
class ReviewQueue:
    """Container for a set of review candidates."""

    candidates: list[ReviewCandidate] = field(default_factory=list)
    total_pending: int = 0
    total_approved: int = 0
    total_rejected: int = 0


def _review_section_path(base: str = "rbforge.reviews") -> str:
    """Return the RBMEM section path for review storage."""
    return base


def queue_candidate(
    tool_name: str,
    candidate_data: dict[str, Any],
    priority: int = 3,
    *,
    store: RbmemStore | None = None,
    memory_path: str = "memory.rbmem",
    rbmem_cli: str | None = None,
    review_section: str = "rbforge.reviews",
) -> ReviewCandidate:
    """Queue a new tool candidate for human review.

    Creates a review entry and persists it to the RBMEM store under
    ``rbforge.reviews.<tool_name>``.

    Args:
        tool_name: Name of the tool being reviewed.
        candidate_data: Full tool specification / metadata.
        priority: Urgency level 1 (highest) to 5 (lowest).
        store: Optional pre-created RbmemStore.
        memory_path: Path to the .rbmem file.
        rbmem_cli: Optional rbmem CLI path.
        review_section: Section prefix for review storage.

    Returns:
        The created :class:`ReviewCandidate`.
    """
    if store is None:
        store = RbmemStore(memory_path, rbmem_cli=rbmem_cli)

    review_id = _generate_review_id(tool_name)
    candidate = ReviewCandidate(
        review_id=review_id,
        tool_name=tool_name,
        status=ReviewStatus.PENDING,
        candidate_data=candidate_data,
        priority=priority,
        queued_at=utc_now_iso(),
        notes=candidate_data.get("review_notes", ""),
        _section_path=f"{review_section}.{tool_name}",
    )

    store.update_section(
        candidate._section_path,
        "json",
        candidate_data,
        actor="rbforge_review",
    )

    return candidate


def _generate_review_id(tool_name: str) -> str:
    """Generate a unique review ID from tool name and random suffix."""
    import uuid
    random_suffix = uuid.uuid4().hex[:8]
    return f"{tool_name}_{random_suffix}"


def get_pending_reviews(
    limit: int = 10,
    *,
    store: RbmemStore | None = None,
    memory_path: str = "memory.rbmem",
    rbmem_cli: str | None = None,
    review_section: str = "rbforge.reviews",
) -> ReviewQueue:
    """Fetch pending review candidates from the store.

    Args:
        limit: Maximum number of candidates to return.
        store: Optional pre-created RbmemStore.
        memory_path: Path to the .rbmem file.
        rbmem_cli: Optional rbmem CLI path.
        review_section: Section prefix for review storage.

    Returns:
        A :class:`ReviewQueue` with pending candidates sorted by
        priority (ascending).
    """
    if store is None:
        store = RbmemStore(memory_path, rbmem_cli=rbmem_cli)

    try:
        payload = store.context(review_section, resolve=True, minified=False, graph_depth=0)
    except Exception:  # noqa: BLE001 - diagnostics fallback
        return ReviewQueue()

    candidates: list[ReviewCandidate] = []
    counts = {"pending": 0, "approved": 0, "rejected": 0}

    for section in payload.get("sections", []):
        path = section.get("path", "")
        if not path.startswith(review_section):
            continue
        content = section.get("content", "{}")
        if isinstance(content, str):
            try:
                content = json.loads(content)
            except json.JSONDecodeError:
                continue
        if not isinstance(content, dict):
            continue

        status_str = content.get("status", "pending")
        try:
            status = ReviewStatus(status_str)
        except ValueError:
            status = ReviewStatus.PENDING

        review_id = content.get("review_id", _generate_review_id(content.get("tool_name", "unknown")))

        candidate = ReviewCandidate(
            review_id=review_id,
            tool_name=content.get("tool_name", ""),
            status=status,
            candidate_data=content,
            priority=int(content.get("priority", 3)),
            queued_at=content.get("queued_at", utc_now_iso()),
            notes=content.get("notes", ""),
            reviewed_by=content.get("reviewed_by"),
            reviewed_at=content.get("reviewed_at"),
            _section_path=path,
        )
        candidates.append(candidate)

        if status == ReviewStatus.PENDING:
            counts["pending"] += 1
        elif status == ReviewStatus.APPROVED:
            counts["approved"] += 1
        elif status == ReviewStatus.REJECTED:
            counts["rejected"] += 1

    candidates.sort(key=lambda c: c.priority)
    return ReviewQueue(
        candidates=candidates[:limit],
        total_pending=counts["pending"],
        total_approved=counts["approved"],
        total_rejected=counts["rejected"],
    )


def update_review(
    review_id: str,
    status: ReviewStatus | str,
    notes: str = "",
    *,
    store: RbmemStore | None = None,
    memory_path: str = "memory.rbmem",
    rbmem_cli: str | None = None,
    review_section: str = "rbforge.reviews",
    reviewed_by: str = "human",
) -> ReviewCandidate | None:
    """Update a review's status and optionally add notes.

    Args:
        review_id: The review ID to update.
        status: New status (string or ReviewStatus enum).
        notes: Review notes to store.
        store: Optional pre-created RbmemStore.
        memory_path: Path to the .rbmem file.
        rbmem_cli: Optional rbmem CLI path.
        review_section: Section prefix for review storage.
        reviewed_by: Name/identifier of the reviewer.

    Returns:
        The updated :class:`ReviewCandidate`, or ``None`` if not found.
    """
    if store is None:
        store = RbmemStore(memory_path, rbmem_cli=rbmem_cli)

    if isinstance(status, str):
        try:
            status = ReviewStatus(status)
        except ValueError:
            raise ValueError(f"invalid review status: {status}")

    # Find the candidate
    queue = get_pending_reviews(
        limit=1000, store=store, memory_path=memory_path, rbmem_cli=rbmem_cli
    )

    candidate = None
    for c in queue.candidates:
        if c.review_id == review_id or c.tool_name == review_id:
            candidate = c
            break

    # If not found in pending, try all sections
    if candidate is None:
        all_sections = store.context(review_section, resolve=False, graph_depth=0)
        for section in all_sections.get("sections", []):
            path = section.get("path", "")
            if review_id in path or review_id.replace("-", "_") in path:
                content = section.get("content", {})
                if isinstance(content, str):
                    content = json.loads(content)
                candidate = ReviewCandidate(
                    review_id=review_id,
                    tool_name=content.get("tool_name", ""),
                    status=ReviewStatus.PENDING,
                    candidate_data=content,
                    priority=3,
                    queued_at=content.get("queued_at", ""),
                    notes=notes,
                    reviewed_by=reviewed_by,
                    reviewed_at=utc_now_iso(),
                    _section_path=path,
                )
                break

    if candidate is None:
        return None

    # Update the section in RBMEM
    content = dict(candidate.candidate_data)
    content["status"] = status.value
    content["review_id"] = candidate.review_id
    content["reviewed_by"] = reviewed_by
    content["reviewed_at"] = utc_now_iso()
    if notes:
        content["review_notes"] = notes

    store.update_section(
        candidate._section_path,
        "json",
        content,
        actor="rbforge_review",
    )

    # Update candidate
    candidate = ReviewCandidate(
        review_id=candidate.review_id,
        tool_name=candidate.tool_name,
        status=status,
        candidate_data=content,
        priority=candidate.priority,
        queued_at=candidate.queued_at,
        notes=notes,
        reviewed_by=reviewed_by,
        reviewed_at=utc_now_iso(),
        _section_path=candidate._section_path,
    )

    return candidate


def review_required(
    tool_name: str,
    *,
    store: RbmemStore | None = None,
    memory_path: str = "memory.rbmem",
    rbmem_cli: str | None = None,
    review_section: str = "rbforge.reviews",
) -> bool:
    """Check if a tool requires human review before execution.

    This is the gate function called by ``forge_tool()`` and
    ``runner.py`` to block execution for high-impact tools that
    have a pending review status.

    Args:
        tool_name: Name of the tool to check.
        store: Optional pre-created RbmemStore.
        memory_path: Path to the .rbmem file.
        rbmem_cli: Optional rbmem CLI path.
        review_section: Section prefix for review storage.

    Returns:
        True if the tool has a pending review that blocks execution.
    """
    if store is None:
        store = RbmemStore(memory_path, rbmem_cli=rbmem_cli)

    try:
        payload = store.context(
            f"{review_section}.{tool_name}",
            resolve=True,
            minified=False,
            graph_depth=0,
        )
    except Exception:  # noqa: BLE001
        return False

    for section in payload.get("sections", []):
        if tool_name in section.get("path", ""):
            content = section.get("content", {})
            if isinstance(content, str):
                content = json.loads(content)
            status = content.get("status", "")
            if status == "pending_review":
                return True

    return False

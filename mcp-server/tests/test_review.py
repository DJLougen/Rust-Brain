"""Tests for human-in-the-loop review queue module."""

import dataclasses
import json
import pathlib
import tempfile
from unittest import mock

import pytest

from rbforge_core.review import (
    ReviewCandidate,
    ReviewQueue,
    ReviewStatus,
    _generate_review_id,
    _review_section_path,
    get_pending_reviews,
    queue_candidate,
    review_required,
    update_review,
    utc_now_iso,
)


def test_utc_now_iso_returns_valid_iso() -> None:
    """utc_now_iso() returns a valid ISO-8601 string ending with Z."""
    iso = utc_now_iso()
    assert isinstance(iso, str)
    assert iso.endswith("Z")
    # Should be parseable (basic check)
    assert "T" in iso


class TestReviewStatus:
    """Tests for the ReviewStatus enum."""

    def test_has_expected_members(self) -> None:
        assert ReviewStatus.PENDING.value == "pending"
        assert ReviewStatus.APPROVED.value == "approved"
        assert ReviewStatus.REJECTED.value == "rejected"
        assert ReviewStatus.NEEDS_REVISION.value == "needs_revision"

    def test_from_string(self) -> None:
        assert ReviewStatus("pending") == ReviewStatus.PENDING
        assert ReviewStatus("approved") == ReviewStatus.APPROVED
        assert ReviewStatus("rejected") == ReviewStatus.REJECTED
        assert ReviewStatus("needs_revision") == ReviewStatus.NEEDS_REVISION

    def test_invalid_string_raises(self) -> None:
        with pytest.raises(ValueError):
            ReviewStatus("bogus")


class TestReviewCandidate:
    """Tests for the ReviewCandidate frozen dataclass."""

    def test_can_be_created(self) -> None:
        data = {"key": "value"}
        candidate = ReviewCandidate(
            review_id="rev-001",
            tool_name="test_tool",
            status=ReviewStatus.PENDING,
            candidate_data=data,
            priority=1,
            queued_at="2026-05-17T10:00:00Z",
            _section_path="rbforge.reviews.test_tool",
        )
        assert candidate.review_id == "rev-001"
        assert candidate.tool_name == "test_tool"
        assert candidate.status == ReviewStatus.PENDING
        assert candidate.candidate_data == data
        assert candidate.priority == 1
        assert candidate._section_path == "rbforge.reviews.test_tool"

    def test_defaults(self) -> None:
        candidate = ReviewCandidate(
            review_id="id",
            tool_name="t",
            status=ReviewStatus.PENDING,
            candidate_data={},
            priority=3,
            queued_at="t",
            _section_path="p",
        )
        assert candidate.notes == ""
        assert candidate.reviewed_by is None
        assert candidate.reviewed_at is None

    def test_frozen_prevents_mutation(self) -> None:
        candidate = ReviewCandidate(
            review_id="id",
            tool_name="t",
            status=ReviewStatus.PENDING,
            candidate_data={},
            priority=3,
            queued_at="t",
            _section_path="p",
        )
        with pytest.raises(Exception):
            candidate.review_id = "new"  # type: ignore[misc]


class TestReviewQueue:
    """Tests for the ReviewQueue dataclass."""

    def test_empty_queue(self) -> None:
        queue = ReviewQueue()
        assert queue.candidates == []
        assert queue.total_pending == 0
        assert queue.total_approved == 0
        assert queue.total_rejected == 0

    def test_counts_reflect_candidates(self) -> None:
        c1 = ReviewCandidate(
            review_id="r1",
            tool_name="a",
            status=ReviewStatus.PENDING,
            candidate_data={},
            priority=1,
            queued_at="t",
            _section_path="p",
        )
        c2 = ReviewCandidate(
            review_id="r2",
            tool_name="b",
            status=ReviewStatus.APPROVED,
            candidate_data={},
            priority=2,
            queued_at="t",
            _section_path="p",
        )
        c3 = ReviewCandidate(
            review_id="r3",
            tool_name="c",
            status=ReviewStatus.REJECTED,
            candidate_data={},
            priority=3,
            queued_at="t",
            _section_path="p",
        )
        queue = ReviewQueue(
            candidates=[c1, c2, c3],
            total_pending=1,
            total_approved=1,
            total_rejected=1,
        )
        assert queue.total_pending == 1
        assert queue.total_approved == 1
        assert queue.total_rejected == 1


class TestReviewIdGeneration:
    """Tests for the review ID generation helper."""

    def test_id_has_tool_name_prefix(self) -> None:
        id1 = _generate_review_id("my_tool")
        assert id1.startswith("my_tool_")

    def test_ids_are_unique_across_calls(self) -> None:
        # Generate two IDs quickly; they should differ
        id1 = _generate_review_id("dup_tool")
        id2 = _generate_review_id("dup_tool")
        assert id1 != id2

    def test_id_includes_random_suffix(self) -> None:
        id1 = _generate_review_id("abc")
        id2 = _generate_review_id("xyz")
        # Format: {tool_name}_{8_hex_chars}
        parts1 = id1.split("_")
        parts2 = id2.split("_")
        assert len(parts1) == 2
        assert len(parts2) == 2
        assert len(parts1[1]) == 8
        assert len(parts2[1]) == 8
        assert all(c in "0123456789abcdef" for c in parts1[1])


class TestQueueCandidate:
    """Tests for queue_candidate()."""

    def test_queue_creates_pending_candidate(self) -> None:
        """queue_candidate() creates a PENDING candidate and returns it."""
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": []}
        candidate = queue_candidate(
            tool_name="test_qc",
            candidate_data={"key": "val"},
            store=mock_store,
        )
        assert isinstance(candidate, ReviewCandidate)
        assert candidate.tool_name == "test_qc"
        assert candidate.status == ReviewStatus.PENDING
        assert candidate.priority == 3
        assert candidate.candidate_data == {"key": "val"}
        # Verify store was called to persist
        mock_store.update_section.assert_called_once()
        call_args = mock_store.update_section.call_args
        assert call_args[0][0] == "rbforge.reviews.test_qc"
        assert call_args[0][2] == {"key": "val"}
        assert call_args[1]["actor"] == "rbforge_review"

    def test_queue_respects_priority(self) -> None:
        """queue_candidate() passes the priority through."""
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": []}
        candidate = queue_candidate(
            tool_name="prio_tool",
            candidate_data={"x": 1},
            priority=1,
            store=mock_store,
        )
        assert candidate.priority == 1


class TestGetPendingReviews:
    """Tests for get_pending_reviews()."""

    def test_returns_empty_when_no_sections(self) -> None:
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": []}
        queue = get_pending_reviews(
            limit=5,
            store=mock_store,
        )
        assert isinstance(queue, ReviewQueue)
        assert len(queue.candidates) == 0
        assert queue.total_pending == 0

    def test_returns_reviews_with_pending_status(self) -> None:
        """Reviews with pending status are returned."""
        sections = [
            {
                "path": "rbforge.reviews.my_tool",
                "content": json.dumps({
                    "tool_name": "my_tool",
                    "status": "pending",
                    "priority": 2,
                    "review_id": "rev-001",
                    "queued_at": "2026-05-17T10:00:00Z",
                }),
            },
        ]
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": sections}
        queue = get_pending_reviews(limit=10, store=mock_store)
        assert len(queue.candidates) >= 1
        assert queue.total_pending >= 1
        pending = [c for c in queue.candidates if c.status == ReviewStatus.PENDING]
        assert len(pending) >= 1
        assert pending[0].tool_name == "my_tool"

    def test_sorts_by_priority(self) -> None:
        """Candidates are sorted by priority ascending (1=highest first)."""
        sections = [
            {
                "path": "rbforge.reviews.high_prio",
                "content": json.dumps({
                    "tool_name": "high",
                    "status": "pending",
                    "priority": 1,
                    "review_id": "rev-002",
                    "queued_at": "t",
                }),
            },
            {
                "path": "rbforge.reviews.low_prio",
                "content": json.dumps({
                    "tool_name": "low",
                    "status": "pending",
                    "priority": 5,
                    "review_id": "rev-003",
                    "queued_at": "t",
                }),
            },
        ]
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": sections}
        queue = get_pending_reviews(limit=10, store=mock_store)
        assert len(queue.candidates) == 2
        assert queue.candidates[0].priority <= queue.candidates[1].priority
        assert queue.candidates[0].tool_name == "high"

    def test_limits_results(self) -> None:
        """limit parameter caps the returned candidates."""
        sections = []
        for i in range(10):
            sections.append({
                "path": f"rbforge.reviews.tool_{i}",
                "content": json.dumps({
                    "tool_name": f"tool_{i}",
                    "status": "pending",
                    "priority": i + 1,
                    "review_id": f"rev-{i:03d}",
                    "queued_at": "t",
                }),
            })
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": sections}
        queue = get_pending_reviews(
            limit=3,
            store=mock_store,
        )
        assert len(queue.candidates) == 3

    def test_counts_all_statuses(self) -> None:
        """total_pending, total_approved, total_rejected count all statuses."""
        sections = [
            {"path": "rbforge.reviews/p1", "content": json.dumps({"tool_name": "p1", "status": "pending", "review_id": "r1", "queued_at": "t"})},
            {"path": "rbforge.reviews/p2", "content": json.dumps({"tool_name": "p2", "status": "pending", "review_id": "r2", "queued_at": "t"})},
            {"path": "rbforge.reviews/a1", "content": json.dumps({"tool_name": "a1", "status": "approved", "review_id": "r3", "queued_at": "t"})},
            {"path": "rbforge.reviews/r1", "content": json.dumps({"tool_name": "r1", "status": "rejected", "review_id": "r4", "queued_at": "t"})},
        ]
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": sections}
        queue = get_pending_reviews(limit=10, store=mock_store)
        assert queue.total_pending == 2
        assert queue.total_approved == 1
        assert queue.total_rejected == 1

    def test_invalid_status_defaults_to_pending(self) -> None:
        """Sections with unrecognized status default to PENDING."""
        sections = [
            {"path": "rbforge.reviews/bad", "content": json.dumps({"tool_name": "bad", "status": "foobar", "review_id": "r5", "queued_at": "t"})},
        ]
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": sections}
        queue = get_pending_reviews(limit=10, store=mock_store)
        assert any(c.status == ReviewStatus.PENDING for c in queue.candidates)

    def test_fallback_on_bad_store(self) -> None:
        """If the store raises an exception, an empty queue is returned."""
        mock_store = mock.Mock()
        mock_store.context.side_effect = RuntimeError("store unavailable")
        queue = get_pending_reviews(
            limit=10,
            store=mock_store,
        )
        assert isinstance(queue, ReviewQueue)
        assert len(queue.candidates) == 0


class TestUpdateReview:
    """Tests for update_review()."""

    def test_update_approves_pending_candidate(self) -> None:
        """update_review() changes PENDING to APPROVED."""
        section_data = [
            {
                "path": "rbforge.reviews.test_update_tool",
                "content": json.dumps({
                    "tool_name": "test_update_tool",
                    "status": "pending",
                    "review_id": "rev-update-001",
                    "priority": 1,
                    "queued_at": "t",
                }),
            },
        ]
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": section_data}
        candidate = update_review(
            review_id="rev-update-001",
            status=ReviewStatus.APPROVED,
            notes="Looks good, approved.",
            store=mock_store,
            reviewed_by="alice",
        )
        assert candidate is not None
        assert candidate.status == ReviewStatus.APPROVED
        assert candidate.reviewed_by == "alice"
        assert "Looks good" in candidate.notes

    def test_update_rejects_candidate(self) -> None:
        """update_review() can change status to REJECTED."""
        section_data = [
            {
                "path": "rbforge.reviews/test_tool",
                "content": json.dumps({
                    "tool_name": "test_tool",
                    "status": "pending",
                    "review_id": "rev-reject-001",
                    "queued_at": "t",
                }),
            },
        ]
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": section_data}
        candidate = update_review(
            review_id="rev-reject-001",
            status="rejected",
            notes="Fails security check.",
            store=mock_store,
            reviewed_by="bob",
        )
        assert candidate is not None
        assert candidate.status == ReviewStatus.REJECTED

    def test_update_not_found_returns_none(self) -> None:
        """update_review() returns None for a nonexistent review ID."""
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": []}
        candidate = update_review(
            review_id="nonexistent-id",
            status=ReviewStatus.APPROVED,
            store=mock_store,
        )
        assert candidate is None

    def test_update_invalid_status_raises(self) -> None:
        """update_review() raises ValueError for invalid status strings."""
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": []}
        with pytest.raises(ValueError, match="invalid review status"):
            update_review(
                review_id="fake-id",
                status="not_a_valid_status",
                store=mock_store,
            )

    def test_update_by_tool_name(self) -> None:
        """update_review() can match by tool_name when review_id is not exact."""
        section_data = [
            {
                "path": "rbforge.reviews/name_tool",
                "content": json.dumps({
                    "tool_name": "name_tool",
                    "status": "pending",
                    "review_id": "rev-nm-001",
                    "queued_at": "t",
                }),
            },
        ]
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": section_data}
        # Pass tool_name as the review_id parameter
        candidate = update_review(
            review_id="name_tool",
            status=ReviewStatus.NEEDS_REVISION,
            store=mock_store,
        )
        assert candidate is not None
        assert candidate.tool_name == "name_tool"


class TestReviewRequired:
    """Tests for the review_required() gate function."""

    def test_no_review_section_returns_false(self) -> None:
        """When there's no review section, the gate returns False."""
        mock_store = mock.Mock()
        mock_store.context.return_value = {"sections": []}
        assert review_required("foo", store=mock_store) is False

    def test_pending_review_blocks_execution(self) -> None:
        """A tool with a pending_review status triggers the gate."""
        mock_store = mock.Mock()
        mock_store.context.return_value = {
            "sections": [
                {
                    "path": "rbforge.reviews.blocked_tool",
                    "content": json.dumps({
                        "tool_name": "blocked_tool",
                        "status": "pending_review",
                        "review_id": "rev-blk-001",
                    }),
                },
            ],
        }
        assert review_required("blocked_tool", store=mock_store) is True

    def test_approved_tool_passes_gate(self) -> None:
        """An approved tool does not trigger the gate."""
        mock_store = mock.Mock()
        mock_store.context.return_value = {
            "sections": [
                {
                    "path": "rbforge.reviews/clean_tool",
                    "content": json.dumps({
                        "tool_name": "clean_tool",
                        "status": "approved",
                        "review_id": "rev-002",
                    }),
                },
            ],
        }
        assert review_required("clean_tool", store=mock_store) is False


class TestReviewSectionPath:
    """Tests for the internal section path helper."""

    def test_default_section_path(self) -> None:
        assert _review_section_path() == "rbforge.reviews"

    def test_custom_section_path(self) -> None:
        assert _review_section_path("custom.reviews") == "custom.reviews"

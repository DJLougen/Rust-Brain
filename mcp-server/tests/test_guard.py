from __future__ import annotations

import time
from typing import Any

from rbforge_core.guard import (
    Budget,
    CircuitBreaker,
    CircuitState,
    ExecutionResult,
    ReliabilityGuard,
    RetryPolicy,
)


def test_budget_default_values() -> None:
    b = Budget()
    assert b.max_tokens == 100_000
    assert b.max_steps == 50
    assert b.timeout_seconds == 300.0


def test_retry_policy_exponential_backoff() -> None:
    policy = RetryPolicy(base_delay=1.0, max_delay=60.0, jitter=False)
    assert policy.next_delay(0) == 1.0
    assert policy.next_delay(1) == 2.0
    assert policy.next_delay(2) == 4.0
    assert policy.next_delay(10) == 60.0  # capped


def test_circuit_breaker_transitions() -> None:
    cb = CircuitBreaker(failure_threshold=3)
    assert cb.state == CircuitState.CLOSED

    cb.record_failure()
    cb.record_failure()
    assert cb.state == CircuitState.CLOSED

    cb.record_failure()  # hits threshold
    assert cb.state == CircuitState.OPEN

    assert cb.can_execute() is False

    # After reset timeout, transitions to half-open
    cb._last_failure_time = time.monotonic() - 120.0
    assert cb.can_execute() is True
    assert cb.state == CircuitState.HALF_OPEN


def test_circuit_breaker_resets_on_success() -> None:
    cb = CircuitBreaker(failure_threshold=2)
    cb.record_failure()
    cb.record_failure()
    assert cb.state == CircuitState.OPEN

    cb.record_success()
    assert cb.state == CircuitState.CLOSED
    assert cb._failure_count == 0


def test_reliability_guard_success() -> None:
    guard = ReliabilityGuard()

    def good_fn() -> dict[str, Any]:
        return {"result": 42}

    result = guard.execute(good_fn)
    assert result.ok is True
    assert result.output == {"result": 42}
    assert result.attempt == 1


def test_reliability_guard_retries_on_failure() -> None:
    call_count = 0

    def flaky_fn() -> dict[str, Any]:
        nonlocal call_count
        call_count += 1
        if call_count < 3:
            raise RuntimeError("transient error")
        return {"result": "recovered"}

    guard = ReliabilityGuard(
        retry_policy=RetryPolicy(max_retries=5, base_delay=0.001, jitter=False)
    )
    result = guard.execute(flaky_fn)
    assert result.ok is True
    assert result.output == {"result": "recovered"}
    assert result.attempt == 3


def test_reliability_guard_exhausts_retries() -> None:
    def bad_fn() -> dict[str, Any]:
        raise ValueError("permanent failure")

    guard = ReliabilityGuard(
        retry_policy=RetryPolicy(max_retries=2, base_delay=0.001, jitter=False)
    )
    result = guard.execute(bad_fn)
    assert result.ok is False
    assert "permanent failure" in str(result.error)
    assert result.attempt == 2


def test_reliability_guard_circuit_breaker_blocks() -> None:
    cb = CircuitBreaker(failure_threshold=2)
    guard = ReliabilityGuard(circuit_breaker=cb)

    def failing_fn() -> dict[str, Any]:
        raise RuntimeError("fail")

    # Cause enough failures to open circuit
    guard.execute(failing_fn)
    guard.execute(failing_fn)
    assert cb.state == CircuitState.OPEN

    # Subsequent calls should be rejected
    result = guard.execute(failing_fn)
    assert result.ok is False
    assert result.circuit_open is True
    assert "circuit breaker is open" in str(result.error)


def test_reliability_guard_respects_timeout_budget() -> None:
    slow_call_count = 0

    def slow_fn() -> dict[str, Any]:
        nonlocal slow_call_count
        slow_call_count += 1
        time.sleep(0.2)
        return {"done": True}

    guard = ReliabilityGuard(
        budget=Budget(timeout_seconds=0.15),
        retry_policy=RetryPolicy(max_retries=3, base_delay=0.001, jitter=False),
    )
    result = guard.execute(slow_fn)
    assert result.ok is False
    assert result.budget_exhausted is True
    assert result.attempt == 1  # timeout hit before retries

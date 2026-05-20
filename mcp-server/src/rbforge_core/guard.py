"""Agent reliability guards: budgets, retries, timeouts, circuit-breaker."""

from __future__ import annotations

import asyncio
import random
import threading
import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Callable


class CircuitState(Enum):
    """State of a circuit-breaker."""

    CLOSED = "closed"
    OPEN = "open"
    HALF_OPEN = "half_open"


@dataclass(frozen=True)
class Budget:
    """Token/step budget for a single tool call."""

    max_tokens: int = 100_000
    max_steps: int = 50
    timeout_seconds: float = 300.0


@dataclass
class RetryPolicy:
    """Retry configuration with exponential backoff."""

    max_retries: int = 3
    base_delay: float = 1.0
    max_delay: float = 60.0
    jitter: bool = True

    def next_delay(self, attempt: int) -> float:
        """Compute the delay for a given retry attempt."""
        delay = min(self.base_delay * (2 ** attempt), self.max_delay)
        if self.jitter:
            delay = delay * (0.5 + random.random() * 0.5)
        return delay


@dataclass
class CircuitBreaker:
    """Circuit-breaker that opens after repeated failures."""

    failure_threshold: int = 5
    reset_timeout: float = 60.0
    state: CircuitState = field(default=CircuitState.CLOSED, init=False)
    _failure_count: int = field(default=0, init=False)
    _last_failure_time: float = field(default=0.0, init=False)

    def record_success(self) -> None:
        self._failure_count = 0
        self.state = CircuitState.CLOSED

    def record_failure(self) -> None:
        self._failure_count += 1
        self._last_failure_time = time.monotonic()
        if self._failure_count >= self.failure_threshold:
            self.state = CircuitState.OPEN

    def can_execute(self) -> bool:
        """Return True if the circuit allows execution."""
        if self.state == CircuitState.CLOSED:
            return True
        if self.state == CircuitState.OPEN:
            elapsed = time.monotonic() - self._last_failure_time
            if elapsed >= self.reset_timeout:
                self.state = CircuitState.HALF_OPEN
                return True
            return False
        # HALF_OPEN: allow one probe request
        return True


@dataclass
class ExecutionResult:
    """Result of a guarded execution attempt."""

    ok: bool
    output: Any | None = None
    error: str | None = None
    attempt: int = 0
    duration_ms: float = 0.0
    budget_exhausted: bool = False
    circuit_open: bool = False


@dataclass
class TimeoutError(Exception):
    """Raised when a guarded execution exceeds its timeout budget."""

    timeout_seconds: float


class _TimeoutThread:
    """Helper that sets an Event on a separate thread to enforce timeouts."""

    def __init__(self, timeout: float) -> None:
        self._event = asyncio.Event()
        self._timeout = timeout
        self._timer: asyncio.TimerHandle | None = None

    def start(self, loop: asyncio.AbstractEventLoop) -> None:
        self._timer = loop.call_later(self._timeout, self._event.set)

    def stop(self) -> None:
        if self._timer is not None:
            self._timer.cancel()
            self._timer = None


class ReliabilityGuard:
    """Wraps tool execution with budgets, retries, timeouts, and circuit-breaking."""

    def __init__(
        self,
        budget: Budget | None = None,
        retry_policy: RetryPolicy | None = None,
        circuit_breaker: CircuitBreaker | None = None,
    ) -> None:
        self.budget = budget or Budget()
        self.retry_policy = retry_policy or RetryPolicy()
        self.circuit_breaker = circuit_breaker or CircuitBreaker()

    def _run_with_timeout(
        self,
        fn: Callable[..., dict[str, Any]],
        *args: Any,
        **kwargs: Any,
    ) -> Any:
        """Run a function with actual timeout enforcement using asyncio."""
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
        timer = _TimeoutThread(self.budget.timeout_seconds)
        result_holder: dict[str, Any] = {"result": None, "exception": None}

        def _target() -> None:
            try:
                result_holder["result"] = fn(*args, **kwargs)
            except Exception as exc:  # noqa: BLE001
                result_holder["exception"] = exc

        timer.start(loop)
        thread = threading.Thread(target=_target, daemon=True)
        thread.start()
        thread.join(timeout=self.budget.timeout_seconds)
        timer.stop()

        if thread.is_alive():
            raise TimeoutError(self.budget.timeout_seconds)

        if result_holder["exception"] is not None:
            raise result_holder["exception"]

        return result_holder["result"]

    def execute(
        self,
        fn: Callable[..., dict[str, Any]],
        *args: Any,
        **kwargs: Any,
    ) -> ExecutionResult:
        """Execute a tool function with all guard policies applied.

        Returns an :class:`ExecutionResult` describing success, failure, or
        rejection due to budget/circuit-breaker limits.
        """
        if not self.circuit_breaker.can_execute():
            return ExecutionResult(
                ok=False,
                error="circuit breaker is open",
                circuit_open=True,
            )

        last_exception: Exception | None = None
        start_time = time.monotonic()

        for attempt in range(1, self.retry_policy.max_retries + 1):
            elapsed = time.monotonic() - start_time
            if elapsed >= self.budget.timeout_seconds:
                return ExecutionResult(
                    ok=False,
                    error="timeout budget exhausted",
                    budget_exhausted=True,
                    attempt=attempt,
                    duration_ms=elapsed * 1000,
                )

            try:
                result = self._run_with_timeout(fn, *args, **kwargs)
                self.circuit_breaker.record_success()
                duration = (time.monotonic() - start_time) * 1000
                return ExecutionResult(ok=True, output=result, attempt=attempt, duration_ms=duration)
            except TimeoutError:
                # Budget exhausted — no retries on timeout
                duration = (time.monotonic() - start_time) * 1000
                return ExecutionResult(
                    ok=False,
                    error="timeout budget exhausted",
                    budget_exhausted=True,
                    attempt=attempt,
                    duration_ms=duration,
                )
            except Exception as exc:  # noqa: BLE001
                last_exception = exc
                self.circuit_breaker.record_failure()
                if attempt < self.retry_policy.max_retries:
                    delay = self.retry_policy.next_delay(attempt)
                    time.sleep(delay)

        duration = (time.monotonic() - start_time) * 1000
        return ExecutionResult(
            ok=False,
            error=str(last_exception) if last_exception else "unknown error",
            attempt=self.retry_policy.max_retries,
            duration_ms=duration,
        )

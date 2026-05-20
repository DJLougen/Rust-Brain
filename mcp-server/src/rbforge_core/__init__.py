"""RBForge public API."""

from rbforge_core.ab_tester import forge_variant, run_ab_test
from rbforge_core.compactor import (
    compact_memory,
    identify_stale_sections,
    distill_section,
    CompactionResult,
    DistillSummary,
)
from rbforge_core.crossfile import (
    CrossFileStore,
    CrossFileResult,
    SectionMergeConflict,
)
from rbforge_core.forge import forge_tool
from rbforge_core.guard import (
    ReliabilityGuard,
    Budget,
    RetryPolicy,
    CircuitBreaker,
    CircuitState,
    ExecutionResult,
    TimeoutError as GuardTimeoutError,
)
from rbforge_core.health import (
    compute_health_score,
    HealthReport,
)
from rbforge_core.improver import improve_tool
from rbforge_core.models import ForgeResult, ToolSpec
from rbforge_core.rbmem import RbmemStore, patch_section_graph
from rbforge_core.review import (
    ReviewQueue,
    ReviewStatus,
    queue_candidate,
    get_pending_reviews,
    update_review,
)
from rbforge_core.runner import run_forged_tool
from rbforge_core.temporal import (
    temporal_query,
    trend_analysis,
    usage_pattern,
    TemporalResult,
)
from rbforge_core.version import RBFORGE_VERSION
from rbforge_core.validation import validate_spec, validate_tool_spec, ToolSpecError, sample_args
from rbforge_core.versioning import (
    create_snapshot,
    list_snapshots,
    restore_snapshot,
    diff_snapshots,
    auto_snapshot_on_forged,
    Snapshot,
    SnapshotDiff,
)

__version__ = RBFORGE_VERSION

__all__ = [
    "ForgeResult",
    "ToolSpec",
    "RbmemStore",
    "__version__",
    # Forging
    "forge_tool",
    "forge_variant",
    "improve_tool",
    "run_ab_test",
    "run_forged_tool",
    # Reliability
    "ReliabilityGuard",
    "Budget",
    "RetryPolicy",
    "CircuitBreaker",
    "CircuitState",
    "ExecutionResult",
    "GuardTimeoutError",
    # Health
    "compute_health_score",
    "HealthReport",
    # Compaction
    "compact_memory",
    "identify_stale_sections",
    "distill_section",
    "CompactionResult",
    "DistillSummary",
    # Cross-file
    "CrossFileStore",
    "CrossFileResult",
    "SectionMergeConflict",
    # Versioning
    "create_snapshot",
    "list_snapshots",
    "restore_snapshot",
    "diff_snapshots",
    "auto_snapshot_on_forged",
    "Snapshot",
    "SnapshotDiff",
    # Temporal
    "temporal_query",
    "trend_analysis",
    "usage_pattern",
    "TemporalResult",
    # Review
    "ReviewQueue",
    "ReviewStatus",
    "queue_candidate",
    "get_pending_reviews",
    "update_review",
    # Validation
    "validate_spec",
    "validate_tool_spec",
    "ToolSpecError",
    "sample_args",
    "patch_section_graph",
]

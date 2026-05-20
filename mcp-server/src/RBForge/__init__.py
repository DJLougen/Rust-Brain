"""RBForge public package.

Example:
    from RBForge import forge_tool, run_forged_tool
"""

from rbforge_core.forge import forge_tool
from rbforge_core.runner import run_forged_tool
from rbforge_core.rbmem import RbmemStore, patch_section_graph
from rbforge_core.validation import validate_spec, validate_tool_spec, ToolSpecError, sample_args
from rbforge_core.models import ToolSpec, ForgeResult
from rbforge_core.version import RBFORGE_VERSION

__version__ = RBFORGE_VERSION

__all__ = [
    "__version__",
    "forge_tool",
    "run_forged_tool",
    "RbmemStore",
    "patch_section_graph",
    "validate_spec",
    "validate_tool_spec",
    "ToolSpecError",
    "sample_args",
    "ToolSpec",
    "ForgeResult",
]

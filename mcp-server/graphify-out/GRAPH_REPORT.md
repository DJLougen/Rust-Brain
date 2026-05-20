# 📊 Graph Analysis Report

**Root:** `.`

## Summary

| Metric | Value |
|--------|-------|
| Nodes | 243 |
| Edges | 342 |
| Communities | 24 |
| Hyperedges | 0 |

### Confidence Breakdown

| Level | Count | Percentage |
|-------|-------|------------|
| EXTRACTED | 224 | 65.5% |
| INFERRED | 118 | 34.5% |
| AMBIGUOUS | 0 | 0.0% |

## 🌟 God Nodes (Most Connected)

| Node | Degree | Community |
|------|--------|-----------|
| forge_tool | 48 | 0 |
| install_hermes_bridge | 17 | 1 |
| rbmem | 16 | 4 |
| test_rbforge_public_api | 15 | 2 |
| forge_tool() | 15 | 3 |
| sandbox | 15 | 6 |
| RbmemStore | 13 | 11 |
| RbmemStore | 13 | 5 |
| run_forged_tool() | 11 | 3 |
| forge | 10 | 8 |

## 🔮 Surprising Connections

- **src_rbforge_forge_tool_py_forge_tool** → **src_rbforge_forge_tool_py_sandbox_validate** (calls)
- **src_rbforge_forge_tool_py_forge_tool** → **src_rbforge_forge_tool_py_failure** (calls)
- **src_rbforge_forge_tool_py_run_forged_tool** → **src_rbforge_forge_tool_py_execute_python_tool** (calls)
- **src_rbforge_forge_tool_py_validate_spec** → **src_rbforge_forge_tool_py_validate_python_source** (calls)
- **src_rbforge_forge_tool_py_validate_spec** → **src_rbforge_forge_tool_py_sample_args** (calls)

## 🏘️ Communities

### Community 0 — validate_python_source() (41 nodes, cohesion: 0.07)

- forge_tool
- _call_name()
- docker_ready()
- execute_python_tool()
- _failure()
- find_or_build_rbmem()
- generated_unittest()
- ast
- dataclasses.asdict
- dataclasses.dataclass
- dataclasses.field
- datetime.timezone
- json
- jsonschema.Draft202012Validator
- jsonschema.ValidationError
- os
- pathlib.Path
- re
- shutil
- subprocess
- _…and 21 more_

### Community 1 — wsl_rbmem_cli() (18 nodes, cohesion: 0.19)

- install_hermes_bridge
- autonomy_lines()
- bridge_config()
- find_rbmem_cli()
- json
- os
- pathlib.Path
- re
- shutil
- subprocess
- yaml
- main()
- remove_legacy_rbmem_sections()
- update_rbmem()
- update_wsl_config()
- update_yaml_config()
- windows_to_wsl_path()
- wsl_rbmem_cli()

### Community 2 — test_web_bubble_tools_can_import_http_clients() (16 nodes, cohesion: 0.13)

- test_rbforge_public_api
- importlib
- pathlib.Path
- RBForge.forge_tool.patch_section_graph
- RBForge.forge_tool.RBForgeError
- RBForge.forge_tool.RbmemStore
- RBForge.forge_tool.run_forged_tool
- RBForge.forge_tool.sample_args
- RBForge.forge_tool.ToolSpec
- RBForge.forge_tool.validate_spec
- test_non_web_tools_still_reject_http_clients()
- test_patch_section_graph_replaces_duplicate_graph_blocks()
- test_public_validate_spec_uses_jsonschema()
- test_run_forged_tool_updates_metrics()
- test_shell_tools_can_import_subprocess_but_other_tools_cannot()
- test_web_bubble_tools_can_import_http_clients()

### Community 3 — validate_spec() (15 nodes, cohesion: 0.28)

- forge_tool()
- .apply_graph()
- .register_tool()
- .update_section()
- .validate()
- _relations()
- _relations_from_record()
- run_forged_tool()
- _tool_record()
- TraceLogger
- .__init__()
- .record()
- _update_metrics()
- utc_now()
- validate_spec()

### Community 4 — _render_graph_block() (15 nodes, cohesion: 0.13)

- rbmem
- find_rbmem_cli()
- json
- rbforge_core.models.ToolSpec
- rbforge_core.models.utc_now_iso
- os
- pathlib.Path
- re
- shutil
- subprocess
- tempfile
- typing.Any
- RbmemError
- .__init__()
- _render_graph_block()

### Community 5 — _tool_relations() (14 nodes, cohesion: 0.40)

- RbmemStore
- .apply_graph()
- .ensure()
- .hermes_load()
- .hermes_save()
- .persist_candidate()
- .read_minified()
- .read_registry()
- .register_validated_tool()
- ._run()
- .update_section()
- .validate()
- tool_record()
- _tool_relations()

### Community 6 — static_warnings() (13 nodes, cohesion: 0.18)

- sandbox
- _call_name()
- generate_python_unittest()
- ast
- json
- rbforge_core.models.SandboxResult
- rbforge_core.models.ToolSpec
- pathlib.Path
- shutil
- subprocess
- tempfile
- _sample_args()
- static_warnings()

### Community 7 — utc_now_iso() (11 nodes, cohesion: 0.18)

- models
- ForgeResult
- dataclasses.dataclass
- dataclasses.field
- datetime.timezone
- typing.Any
- typing.Literal
- SandboxResult
- ToolSpec
- .section_path()
- utc_now_iso()

### Community 8 — _message() (11 nodes, cohesion: 0.20)

- forge
- forge_tool()
- rbforge_core.models.ForgeResult
- rbforge_core.models.ToolSpec
- rbforge_core.rbmem.RbmemStore
- rbforge_core.sandbox.SandboxExecutor
- rbforge_core.trajectory.TrajectoryLogger
- rbforge_core.validation.validate_tool_spec
- pathlib.Path
- typing.Any
- _message()

### Community 9 — validate_tool_spec() (9 nodes, cohesion: 0.28)

- validation
- ast
- rbforge_core.models.ToolSpec
- re
- typing.Any
- ToolSpecError
- _validate_python_source()
- _validate_schema_shape()
- validate_tool_spec()

### Community 10 — main() (10) (9 nodes, cohesion: 0.31)

- demo_invention_loop
- _compact_forge_result()
- _extract_think()
- _extract_tool_call()
- json
- pathlib.Path
- RBForge.forge_tool
- RBForge.run_forged_tool
- main()

### Community 11 — RbmemStore (8 nodes, cohesion: 0.46)

- RbmemStore
- .ensure()
- .hermes_load()
- .hermes_save()
- .load_tool_record()
- .read_minified()
- .read_registry()
- ._run()

### Community 12 — TrajectoryLogger (8 nodes, cohesion: 0.25)

- trajectory
- json
- rbforge_core.models.utc_now_iso
- pathlib.Path
- typing.Any
- TrajectoryLogger
- .__init__()
- .record()

### Community 13 — typing.Any (8 nodes, cohesion: 0.25)

- harness
- json
- rbforge_core.rbmem.RbmemStore
- pathlib.Path
- re
- subprocess
- tempfile
- typing.Any

### Community 14 — SandboxExecutor (7 nodes, cohesion: 0.43)

- _docker_is_ready()
- _run()
- SandboxExecutor
- .__init__()
- ._run_docker()
- ._run_local()
- .validate()

### Community 15 — test_apply_graph_inserts_relations_without_touching_temporal() (7 nodes, cohesion: 0.29)

- test_rbmem_graph_patch
- rbforge_core.rbmem.RbmemStore
- pathlib.Path
- NoopRbmemStore
- .__init__()
- .validate()
- test_apply_graph_inserts_relations_without_touching_temporal()

### Community 16 — test_validation_rejects_bad_name() (7 nodes, cohesion: 0.29)

- test_validation_and_sandbox
- rbforge_core.models.ToolSpec
- rbforge_core.sandbox.SandboxExecutor
- rbforge_core.validation.ToolSpecError
- rbforge_core.validation.validate_tool_spec
- test_valid_python_tool_passes_local_sandbox()
- test_validation_rejects_bad_name()

### Community 17 — ToolHarness (6 nodes, cohesion: 0.40)

- ToolHarness
- .call_forged()
- .debugger_summary()
- .__init__()
- ._load_tool_record()
- .ripgrep()

### Community 18 — main() (18) (5 nodes, cohesion: 0.40)

- starter_harness
- rbforge_core.forge_tool
- rbforge_core.harness.ToolHarness
- pathlib.Path
- main()

### Community 19 — rbforge_core.models.ToolSpec (4 nodes, cohesion: 0.50)

- __init__
- rbforge_core.forge.forge_tool
- rbforge_core.models.ForgeResult
- rbforge_core.models.ToolSpec

### Community 20 — main() (3 nodes, cohesion: 0.67)

- demo
- rbforge_core.forge.forge_tool
- main()

### Community 21 — main() (21) (3 nodes, cohesion: 0.67)

- demo_before_after
- pathlib.Path
- main()

### Community 22 — RBForge.forge_tool.run_forged_tool (3 nodes, cohesion: 0.67)

- __init__
- RBForge.forge_tool.forge_tool
- RBForge.forge_tool.run_forged_tool

### Community 23 — rbforge_core.forge_tool (2 nodes, cohesion: 1.00)

- forge_thread_tool
- rbforge_core.forge_tool

## 🕳️ Knowledge Gaps

No isolated nodes.

**Thin communities** (< 3 nodes): 1 communities

## 💰 Token Cost

| File | Tokens |
|------|--------|
| input | 0 |
| output | 0 |
| **Total** | **0** |

## ❓ Suggested Questions

1. How does 'src_rbforge_forge_tool_py_rbmemstore' relate to 3 different communities (validate_python_source(), RbmemStore, validate_spec())?
1. How does 'src_rbforge_forge_tool_py_rbmemstore_apply_graph' relate to 3 different communities (validate_python_source(), RbmemStore, validate_spec())?
1. How does 'src_rbforge_forge_tool_py_run_forged_tool' relate to 3 different communities (validate_spec(), validate_python_source(), RbmemStore)?
1. How does 'src_rbforge_forge_tool_py_forge_tool' relate to 3 different communities (validate_spec(), RbmemStore, validate_python_source())?
1. How does 'src_rbforge_forge_tool_py' relate to 3 different communities (RbmemStore, validate_spec(), validate_python_source())?
1. Can you verify the inferred relationships of 'forge_tool()' (degree 15)?
1. Why is 'TrajectoryLogger' (8 nodes) loosely connected (cohesion 0.25)? Should it be split?

---
_Generated by graphify-rs_

# Community 10: main() (10)

**Members:** 9

## Nodes

- **demo_invention_loop** (`scripts_demo_invention_loop_py`, File, degree: 8)
- **_compact_forge_result()** (`scripts_demo_invention_loop_py_compact_forge_result`, Function, degree: 2)
- **_extract_think()** (`scripts_demo_invention_loop_py_extract_think`, Function, degree: 2)
- **_extract_tool_call()** (`scripts_demo_invention_loop_py_extract_tool_call`, Function, degree: 2)
- **json** (`scripts_demo_invention_loop_py_import_json`, Module, degree: 1)
- **pathlib.Path** (`scripts_demo_invention_loop_py_import_pathlib_path`, Module, degree: 1)
- **RBForge.forge_tool** (`scripts_demo_invention_loop_py_import_rbforge_forge_tool`, Module, degree: 1)
- **RBForge.run_forged_tool** (`scripts_demo_invention_loop_py_import_rbforge_run_forged_tool`, Module, degree: 1)
- **main()** (`scripts_demo_invention_loop_py_main`, Function, degree: 4)

## Relationships

- scripts_demo_invention_loop_py → scripts_demo_invention_loop_py_import_json (imports)
- scripts_demo_invention_loop_py → scripts_demo_invention_loop_py_import_pathlib_path (imports)
- scripts_demo_invention_loop_py → scripts_demo_invention_loop_py_import_rbforge_forge_tool (imports)
- scripts_demo_invention_loop_py → scripts_demo_invention_loop_py_import_rbforge_run_forged_tool (imports)
- scripts_demo_invention_loop_py → scripts_demo_invention_loop_py_main (defines)
- scripts_demo_invention_loop_py → scripts_demo_invention_loop_py_extract_tool_call (defines)
- scripts_demo_invention_loop_py → scripts_demo_invention_loop_py_extract_think (defines)
- scripts_demo_invention_loop_py → scripts_demo_invention_loop_py_compact_forge_result (defines)
- scripts_demo_invention_loop_py_main → scripts_demo_invention_loop_py_extract_think (calls)
- scripts_demo_invention_loop_py_main → scripts_demo_invention_loop_py_compact_forge_result (calls)
- scripts_demo_invention_loop_py_main → scripts_demo_invention_loop_py_extract_tool_call (calls)


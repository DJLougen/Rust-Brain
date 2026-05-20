# Community 9: validate_tool_spec()

**Members:** 9

## Nodes

- **validation** (`src_rbforge_core_validation_py`, File, degree: 8)
- **ast** (`src_rbforge_core_validation_py_import_ast`, Module, degree: 1)
- **rbforge_core.models.ToolSpec** (`src_rbforge_core_validation_py_import_rbforge_core_models_toolspec`, Module, degree: 1)
- **re** (`src_rbforge_core_validation_py_import_re`, Module, degree: 1)
- **typing.Any** (`src_rbforge_core_validation_py_import_typing_any`, Module, degree: 1)
- **ToolSpecError** (`src_rbforge_core_validation_py_toolspecerror`, Class, degree: 1)
- **_validate_python_source()** (`src_rbforge_core_validation_py_validate_python_source`, Function, degree: 2)
- **_validate_schema_shape()** (`src_rbforge_core_validation_py_validate_schema_shape`, Function, degree: 2)
- **validate_tool_spec()** (`src_rbforge_core_validation_py_validate_tool_spec`, Function, degree: 3)

## Relationships

- src_rbforge_core_validation_py → src_rbforge_core_validation_py_import_ast (imports)
- src_rbforge_core_validation_py → src_rbforge_core_validation_py_import_re (imports)
- src_rbforge_core_validation_py → src_rbforge_core_validation_py_import_typing_any (imports)
- src_rbforge_core_validation_py → src_rbforge_core_validation_py_import_rbforge_core_models_toolspec (imports)
- src_rbforge_core_validation_py → src_rbforge_core_validation_py_toolspecerror (defines)
- src_rbforge_core_validation_py → src_rbforge_core_validation_py_validate_tool_spec (defines)
- src_rbforge_core_validation_py → src_rbforge_core_validation_py_validate_schema_shape (defines)
- src_rbforge_core_validation_py → src_rbforge_core_validation_py_validate_python_source (defines)
- src_rbforge_core_validation_py_validate_tool_spec → src_rbforge_core_validation_py_validate_schema_shape (calls)
- src_rbforge_core_validation_py_validate_tool_spec → src_rbforge_core_validation_py_validate_python_source (calls)


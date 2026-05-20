# Changelog

All notable changes to RBForge will be documented here.

## Unreleased

### Added

- Added Phase 2 runtime building blocks: tool improvement proposals, dependency resolution, A/B testing, structured telemetry, version compatibility checks, and resource limit helpers.
- Added a `ToolRunner` abstraction with CPython, Deno/TypeScript, and optional Wasmtime runners.
- Added dependency-aware `run_forged_tool(..., resolve_dependencies=True)` in `rbforge_core.runner`.
- Added `rbforge improve <tool> <memory.rbmem> --propose-only|--auto-apply`.
- Added an HTTP client for Rust-Brain server mode.
- Added registry audit/deprecation helpers and signed marketplace import/export helpers.
- Added optional MCP server scaffolding with `scripts/mcp_server.py`.
- Bumped RBForge to `1.0.0` and added an explicit `rbmem >= 1.4.0` compatibility check.
- Added a local cross-project compatibility test for the Rust-Brain `rbmem` CLI when it is available.
- Added P3 hardening tests for signed marketplace round trips, tamper rejection, registry archival, HTTP client request/error behavior, and MCP handlers without requiring optional MCP dependencies.

### Changed

- Registry audit now archives weak or stale tools, tombstones the active tool record, and removes archived tools from the active registry.
- MCP server logic now has pure handlers that can be tested independently from the optional MCP transport package.

## [0.6.0] - 2026-05-01

### Added

- Expanded the debugger eval benchmark from 3 cases to 15 cases across
  traceback, test, lock, config, data, API, filesystem, async, CLI, cache, and
  RBMEM graph debugging families.
- Added per-family debugger eval metrics.

## [0.5.0] - 2026-05-01

### Added

- Added `rbforge eval debugger` for deterministic debugger-first vs
  no-debugger benchmark reporting.
- Added debugger eval fixtures with root-cause, turn-reduction, and reusable
  debugger metrics.

## [0.4.0] - 2026-05-01

### Added

- Added deterministic debugger signal extraction for logs, tracebacks, failing
  tests, suspect files, and exception types.
- Added debugger-specific `rbforge doctor` metrics for debugger tool count,
  validation rate, and average success rate.
- Added debugger-use reward shaping and a debugger RL trace fixture.

## [0.3.0] - 2026-05-01

### Added

- Added `rbforge doctor` for one-command RBForge and RBMEM health checks.
- Reports RBForge version, RBMEM CLI version, memory file health, registry size,
  forged tool count, validation rate, and average success rate.
- Added JSON output with the `rbforge.doctor.v1` schema for agent-readable
  diagnostics.

## [0.2.0] - 2026-05-01

### Added

- Integrated RBMEM v0.4 JSON diagnostics through `RbmemStore.doctor()`.
- Integrated RBMEM v0.4 JSON context retrieval through `RbmemStore.context()`.
- Added RBMEM CLI version checks with `RbmemStore.rbmem_version()`.
- Included RBMEM diagnostics and task-specific context previews in forge results.

### Changed

- Renamed the internal prototype package from the old working name to `rbforge_core`.
- Removed old working-name references from code, docs, examples, configs, and generated graph reports.
- Updated persisted RBMEM record schemas to `rbforge.*`.

## [0.1.0] - 2026-05-01

### Added

- Initial RBForge runtime tool forging, validation, sandboxing, RBMEM persistence, registry, graph patching, and usage metrics.

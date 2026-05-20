# RBForge 1.0.0 Release Notes

## Highlights

- RBForge now requires Rust-Brain/RBMEM `1.4.0` or newer.
- Dependency-aware forged tool execution resolves declared dependencies in
  topological order and passes their outputs as `rbforge_context`.
- Runtime execution is organized behind a `ToolRunner` abstraction with Python,
  Deno/TypeScript, and optional Wasmtime runners.
- Tool improvement proposals, A/B testing, resource limit helpers, structured
  telemetry, registry audit/deprecation, marketplace import/export, and MCP
  server scaffolding are available.
- The starter harness now uses the dependency-aware runner and exposes
  improvement, A/B testing, and registry audit helpers.

## Compatibility

The legacy `RBForge` facade remains importable. New code should prefer
`rbforge_core` APIs for dependency resolution, runner selection, telemetry, and
server integration.

## Validation

Validated with:

- `ruff check .`
- `pytest`
- Real smoke flow using the local Rust-Brain `rbmem` CLI to forge a tool, run
  it, improve it, A/B test it, export/import it, audit the registry, and run
  doctor compatibility checks.

# Changelog

All notable changes to this project will be documented here.

## Unreleased

### Added

- Added an explicit `rbmem` library target with public APIs for `load`, `save`, `create`, `read`, `update`, `query`, `context`, and `diff`.
- Added a direct library integration test covering create/update/read/query/context without spawning the CLI.
- Added a cross-project implementation plan in `docs/IMPLEMENTATION_PLAN.md`.
- Added section-level AES-256-GCM encryption with `encrypt`, `decrypt`, and read/query `--decrypt` support.
- Added typed diff rendering and section-level three-way merge with `ours`, `theirs`, `union`, and `manual` strategies.
- Added `SectionIndex` with keyword, path-prefix, graph adjacency, mtime validation, and disk-cache helpers.
- Added graph export formats for DOT, Mermaid, Cytoscape JSON, and GEXF.
- Added Axum HTTP server mode with health, memory, section, query, context, diff, merge, and export routes.
- Added `update --dry-run`, `delete-section`, `hermes save --json-file`, and `migrate`.
- Added source content hashes for Markdown sync provenance and `_source_version` tracking for normalized documents.
- Added parser regression coverage for common LLM-produced RBMEM variants.
- Added configurable graph inference strategies for Markdown conversion, sync, and `rbmem infer`.
- Added `--log-format json|text` backed by `tracing` for structured CLI observability.
- Added RBMEM format version constants and bumped the CLI/library crate to `1.4.0`.

### Changed

- Refactored the core create/read/update/query/context/diff CLI paths to call the library API.
- Markdown title slugs now use dotted paths consistently.
- `hermes:memory` writes are enforced as append-only.
- New writes use RBMEM `1.4.0`; v1.3/v1.3.0 files still parse and normalize with `_source_version`.

## [0.4.0] - 2026-05-01

### Added

- Phase 3 machine-readable diagnostics with `--format json` for `rbmem doctor` and `rbmem hermes doctor`.
- Phase 4 machine-readable context assembly with `--format json` for `rbmem query`, `rbmem context`, and `rbmem pack`.

## [0.3.0] - 2026-05-01

### Added

- Phase 2 diagnostics with `rbmem doctor` and `rbmem hermes doctor` for CLI, file, validation, and Hermes-load checks.

## [0.2.1] - 2026-05-01

### Fixed

- Added `rbmem --version` so Hermes and release checks can distinguish the CLI package version from the RBMEM v1.3 document format version.

## [0.2.0] - 2026-05-01

### Added

- Task-specific context assembly with `query` and `context` commands.
- Named context packs through `.rbmempacks` and the `pack` command.
- Optional per-section provenance with `source.kind`, `source.path`, and `source.actor`.
- Section-level `diff` and review-oriented `review` commands.
- Hermes, Markdown sync, and CLI update provenance stamping.
- RBMEM-backed Hermes/GEPA self-evolution workflow documentation and a validating example memory file.
- Recommended `evolution.*` section schema for candidate skills, diffs, reports, traces, metadata, and skill history.

## [0.1.0] - 2026-04-27

### Added

- RBMEM v1.3 parser with forgiving repair behavior and protected timestamp policy.
- Hierarchical section paths, smart merge, graph views, tree views, timeline views, pruning, and validation.
- Markdown conversion with heading hierarchy preservation.
- Compact and minified resolved output modes for LLM context.
- Relation inference with confidence scores and manual relation preservation.
- Markdown folder sync command with dry-run and watch mode.
- Hermes Harness commands for load, save, init, and watch.
- README, license, contribution guide, GitHub templates, CI, release workflow, and architecture notes.

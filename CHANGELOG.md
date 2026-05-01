# Changelog

All notable changes to this project will be documented here.

## Unreleased

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

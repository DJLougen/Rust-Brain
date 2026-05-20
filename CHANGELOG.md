# Changelog

All notable changes to this project will be documented here.

## [1.4.3] - 2026-05-20

### Added

- Comprehensive encryption edge case tests (27 new tests: empty sections, unicode, large sections, concurrent encryption, wrong-key failures)
- Concurrent operation tests (10 new tests: multi-threaded reads, writes, queries, graph operations)
- Planner stress test benchmarks (chain problems, mixed constraints, dense conflicts, cube-and-conquer comparison)
- CI/CD pipeline with GitHub Actions: automated testing, Python coverage reporting via Codecov, dependency caching
- Release automation: cross-platform binaries (Windows/Linux/macOS) on version tag push
- Expanded lib.rs API documentation with module overview and usage examples

### Changed

- Fixed all Clippy warnings: derivable_impls, for_kv_map, sort_by_key, too_many_arguments, dead code
- Refactored `persist_plan` to use `PersistPlanArgs` struct (eliminates 9-argument function)

### Performance

- **DPLL Trail-based Backtracking**: Replaced assignment cloning with trail stack for precise backtracking, eliminating O(n) allocations per decision branch
- **Candidate Normalization Caching**: Added `normalized_title` and `normalized_text` fields to `CandidateAction`, pre-computed during `build_problem()` to avoid redundant `normalize()` calls
- **Path Matching Optimization**: Split directly on delimiters instead of full `normalize()` allocation in constraint checking
- **Parser CRLF Check**: Check for `\r\n` before replacing to avoid copy for Unix-formatted documents
- **Index Adjacency Construction**: Incremental path building vs O(n²) slice joins for hierarchical relationships
- Planner execution: **18% faster** for small problems (2,552µs → 2,093µs), **22% faster** for large problems (99,202µs → 77,372µs)
- Index build: **10% faster** (286µs → 258µs)
- Document parsing: **6% faster** (415µs → 390µs)

### Test Results

- Rust: **107 tests passing** (up from 69), 14 test suites
- Python: **178 tests passing** (mcp-server)
- Total: **285 tests** across both codebases

## [1.4.2] - 2026-05-20

### Added

- **Merged RBForge MCP server** into `mcp-server/` directory (178 Python tests)
  - Runtime tool creation system for AI agents via Model Context Protocol
  - Forge, validate, and persist custom tools into RBMEM
  - Tool registry with metrics, indexing, and graph relationships
  - Works with Claude, Cursor, and other MCP-compatible agents
  - Comprehensive documentation: HOWTO, API reference, architecture guide
- Extracted `main.rs` business logic into library modules (`hermes.rs`, `sync.rs`, `pack.rs`, `markdown.rs`)
- Added `--max-tokens` CLI flag for query, context, and pack commands with priority-based truncation
- Added benchmark suite comparing RBMEM vs Markdown across 5 dimensions
- Added benchmark infographic showing precision, recall, token efficiency, and graph-aware retrieval

### Changed

- Wired `SectionIndex` into query path for O(1) keyword lookups instead of linear scan
- Improved query scoring with content-length normalization, recency bonus, and path-depth weighting
- Enhanced relation inference to reject negated matches (20 negation words)
- Rewrote DPLL solver with VSIDS variable ordering, decay, and restart heuristics
- Replaced O(n²) conflict counting with HashMap grouping in health reports
- Removed dead code (`query_matches`, `include_graph_neighbors` from `commands.rs`)
- Updated README with professional structure, banner, and architecture diagram

### Performance

- **5.7× query speedup**: 62µs/query (down from 356µs) with cached index support
- Added `query_document_with_index` and `query_document_with_budget_and_index` for index reuse across queries
- Pre-compute `Utc::now()` once per query instead of per-section (eliminates n syscalls)
- Optimized `SectionIndex::build` to avoid `format!` allocation by tokenizing path and content separately
- Used `HashMap` for token estimation in budget truncation (O(n) vs O(k×n))
- Added `SectionIndex::contains_path` using allocation-free binary search
- Replaced `include_parent_sections` with `include_parent_sections_indexed` using index binary search
- Eliminated redundant `known_paths` BTreeSet rebuild in graph traversal
- Query latency now only 1.9× slower than plain grep while providing graph-aware, temporally-ranked results

## [1.4.1] - 2026-05-18

### Added

- Snapshot JSON serialization with `serde_json::to_string_pretty` for `.snap` metadata
- Configurable `--stale-days` for health reports (default: 90 days)
- `HealthScore` export with scoring formula
- `SectionType::Guards` and `SectionType::Review` variants
- Review `--dry-run` flag for validation without writes
- Snapshot listing with `rbmem snapshot list`

### Fixed

- Switched from YAML to JSON for snapshot metadata to handle edge cases (colons, quotes, Unicode)
- Review dry-run borrow-before-move fix for `warning_count`

## Unreleased (merged into 1.4.2)

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

# Rust-Brain + RBForge Implementation Plan

This plan keeps Rust-Brain as the format authority and moves RBForge toward an agent tool runtime that can use Rust-Brain over a stable API. Work is split into compileable milestones so each stage can ship independently.

## Phase 1: Rust-Brain 1.4 Foundation

### Milestone 1: Library Crate Restructure

Goal: make the existing CLI behavior available as a Rust library without changing RBMEM v1.3 file compatibility or CLI commands.

- Add a public command API in `src/lib.rs` for `load`, `save`, `create`, `read`, `update`, `query`, `context`, and the document-level helpers needed by current commands.
- Keep `src/main.rs` responsible for CLI parsing, stdout/stderr, and process exit only.
- Return `Result<T, RbmemError>` from library functions and avoid `process::exit` in the library path.
- Add `Debug`, `Clone`, and `PartialEq` to public API option/result structs.
- Preserve v1.3 parser and serializer behavior exactly.
- Add integration tests that call the library directly and compare CLI-compatible output.

### Milestone 2: Section-Level Encryption

Goal: protect individual sections while leaving the surrounding document readable and mergeable.

- Add `SectionType::Encrypted` while preserving v1.3 parsing for existing files.
- Implement AES-256-GCM encryption/decryption in `src/crypto.rs` using `ring`.
- Resolve keys in priority order: `RBMEM_ENCRYPTION_KEY`, `~/.rbmem/key`, interactive prompt.
- Store encrypted payload metadata as `nonce`, `ciphertext`, and `encrypted_at`.
- Add `rbmem encrypt --section`, `rbmem decrypt --section`, and `--decrypt` for read/query paths.
- Skip encrypted sections in normal read/query results unless decryption is explicitly requested.

### Milestone 3: Diff/Merge Engine

Goal: support collaboration and branch reconciliation at section granularity.

- Add typed `SectionDiff` and `RbmemDiff` models.
- Implement two-way diff for add/remove/type/content/metadata changes.
- Implement three-way merge over base/local/remote with `ours`, `theirs`, `union`, and `manual` strategies.
- Emit RBMEM conflict sections with `type: conflict`, `conflict_at`, `local_version`, and `remote_version`.
- Add text, JSON, and YAML-compatible output renderers.

### Milestone 4: Query Performance Index

Goal: make large `.rbmem` files fast enough for repeated agent queries.

- Build an in-memory inverted keyword index.
- Add a path trie for prefix queries such as `agents.*`.
- Add graph adjacency lists for `related()` breadth-first search up to depth `N`.
- Invalidate cached indexes when file modified time changes.
- Add optional `.rbmem.index` disk cache after correctness tests are stable.

### Milestone 5: Graph Visualization Export

Goal: export RBMEM graph structure into common visualization tools.

- Add exporters for DOT, Mermaid, Cytoscape JSON, and GEXF.
- Include implicit hierarchy edges and explicit graph relations such as `depends_on` and `categorized_as`.
- Add `rbmem export memory.rbmem --format dot|mermaid|cytoscape|gexf`.

### Milestone 6: HTTP Server Mode

Goal: give RBForge and other tools a stable programmatic interface without shelling out.

- Add Axum server under `src/server/mod.rs`.
- Store loaded memories in an `AppState` guarded by `tokio::sync::RwLock`.
- Add endpoints for health, memory CRUD, section CRUD, query, context, diff, merge, and export.
- Add request/response JSON types with explicit error mapping.

## Phase 2: RBForge Runtime Expansion

### Milestone 7: RBForge HTTP Client + Version Gate

- Add `rbforge_core/rbmem_client.py` for Rust-Brain server calls.
- Keep subprocess fallback in CLI only.
- Add version constants and require `rbmem >= 1.4.0` while accepting v1.3 files.

### Milestone 8: Resource-Limited Multi-Language Runners

- Introduce `ToolRunner` ABC.
- Move existing Python execution into `PythonRunner`.
- Add optional `WasmRunner` using `wasmtime`.
- Add `DenoRunner` with `deno run --allow-none`.
- Enforce per-category CPU, memory, file-size, and timeout limits.

### Milestone 9: Dependencies and Variants

- Store declared dependencies in tool metadata.
- Resolve dependencies by topological sort and detect cycles with `CircularDependencyError`.
- Pass dependency results as context to dependent tools.
- Add variants and A/B testing with duration, success, and correctness comparison.

### Milestone 10: Auto-Improvement and Deprecation

- Track recent failures and success rates.
- Detect common Python error families and propose focused improvements.
- Store tool versions under `tools.custom.{name}.versions[]`.
- Audit tools for low success rate or inactivity and archive with full history.

### Milestone 11: Observability, MCP, and Marketplace

- Add structured JSON logging in Rust-Brain via `tracing`.
- Add JSON Lines logging in RBForge via `structlog`.
- Store telemetry under `telemetry.events`.
- Add MCP resources and tools for registry, forge, run, query, and update operations.
- Add signed tool export/import using Ed25519 signatures.

## End-to-End Validation

- Keep v1.3 fixtures loading throughout all milestones.
- Add Rust integration tests for encryption, diff/merge, export, and server endpoints.
- Add Python tests for each RBForge runtime feature.
- Finish with cross-project tests that forge tools into `.rbmem`, query them through the Rust-Brain server, run them through RBForge, and verify telemetry.

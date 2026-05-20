# Architecture

`rbmem` is a single-crate Rust project with both a CLI binary and a library. The codebase is organized into focused modules:

| Module | Responsibility |
|--------|----------------|
| `src/main.rs` | CLI entry point, command parsing, user interaction |
| `src/lib.rs` | Library exports and public API |
| `src/parser.rs` | Forgiving Nom parser for RBMEM v1.3/v1.4 syntax |
| `src/document.rs` | Core data model, smart merge, timestamp protection, graph view, validation |
| `src/commands.rs` | Command implementations (query, context, pack, diff, health) |
| `src/hermes.rs` | Hermes agent integration, payload handling, starter docs |
| `src/planner/` | SAT planning engine with Kissat/CaDiCaL support and DPLL fallback |
| `src/sync.rs` | Markdown folder sync with watch mode |
| `src/pack.rs` | Context pack assembly and configuration |
| `src/markdown.rs` | Markdown to RBMEM conversion |
| `src/server/` | Axum HTTP server with REST API |
| `src/index.rs` | `SectionIndex` for fast keyword and graph lookups |
| `src/diff.rs` | Typed diff and three-way merge |
| `src/crypto.rs` | AES-256-GCM per-section encryption and decryption |
| `src/export.rs` | Graph export to DOT, Mermaid, Cytoscape JSON, and GEXF |
| `src/version.rs` | RBMEM format version constants |
| `mcp-server/` | RBForge MCP server for runtime tool creation (Python) |

## Data Flow

```
┌─────────────┐
│  CLI Input  │
└──────┬──────┘
       │
       ▼
┌─────────────────┐
│  main.rs        │  Parse commands, handle I/O
└──────┬──────────┘
       │
       ▼
┌─────────────────┐
│  Library API    │  hermes, planner, sync, pack, markdown
└──────┬──────────┘
       │
       ▼
┌─────────────────┐
│  commands.rs    │  Query, context, diff, health
└──────┬──────────┘
       │
       ▼
┌─────────────────┐
│  document.rs    │  RbmemDocument, sections, graph, temporal
└──────┬──────────┘
       │
       ▼
┌─────────────────┐
│  parser.rs      │  Parse .rbmem files
└──────┬──────────┘
       │
       ▼
┌─────────────────┐
│  Storage        │  .rbmem files, SectionIndex, HTTP server
└─────────────────┘
```

## Core Components

### RbmemDocument

The central data structure representing a parsed `.rbmem` file:

```rust
pub struct RbmemDocument {
    pub meta: Meta,
    pub sections: Vec<Section>,
    pub warnings: Vec<String>,
    pub graph: Option<GraphView>,
    pub source_version: Option<String>,
}
```

### Section

Individual memory sections with stable paths:

```rust
pub struct Section {
    pub path: String,
    pub section_type: SectionType,
    pub content: String,
    pub temporal: TemporalInfo,
    pub source: Option<SourceInfo>,
    pub graph: Option<SectionGraph>,
    pub parent: Option<String>,
}
```

### SectionIndex

Fast lookups for query and context assembly:

- **Keyword index**: Inverted index for O(1) term lookups
- **Path prefix**: Hierarchical section navigation
- **Graph adjacency**: Follow relationships between sections
- **Disk cache**: Persisted index for large documents

## Context Assembly

Query and context commands select sections through multiple strategies:

1. **Keyword matching**: Inverted index lookup from `SectionIndex`
2. **Path filtering**: Include/exclude by section path patterns
3. **IDF-weighted scoring**: Inverse document frequency boosts rare terms, suppresses common ones
4. **Graph traversal**: Follow edges to related sections (configurable depth)
5. **Temporal filtering**: Prefer recent sections, exclude expired
6. **Precision cutoffs**: Relative score threshold (35% of top), top-K match limiting
7. **Budget truncation**: `--max-tokens` with priority-based selection

## SAT Planning

The planner module (`src/planner/`) converts RBMEM goals and constraints into SAT problems:

1. **Extract actions**: Parse `goals`, `tasks`, `actions` sections
2. **Parse rules**: Convert natural language rules to clauses
   - `A requires B` → `¬A ∨ B`
   - `A conflicts with B` → `¬A ∨ ¬B`
   - `must A` → `A`
   - `avoid A` → `¬A`
3. **Solve**: Use Kissat/CaDiCaL (external) or DPLL (internal with VSIDS)
4. **Write plan**: Store results in `plans.<goal>.<timestamp>.*` sections

### VSIDS Heuristic

The internal DPLL solver uses Variable State Independent Decaying Sum (VSIDS):

- **Activity tracking**: Bump activity for variables in learned clauses
- **Decay**: Multiply all activities by 0.95 after each conflict
- **Restarts**: Restart every 100 conflicts with best-scoring variables
- **Selection**: Choose highest-activity unassigned variable

## Graph Model

RBMEM graphs combine three edge sources:

1. **Implicit edges**: Parent-child from dotted paths (`project.rules` contains `project.rules.testing`)
2. **Manual edges**: Explicit `graph.relations` in section metadata
3. **Inferred edges**: NLP-based relation detection with confidence scores

Inference strategies:
- `off`: No inference
- `explicit`: Only direct relation verbs ("uses", "depends on")
- `balanced`: Default heuristic
- `aggressive`: Lower confidence threshold for broader recall

Negation detection prevents spurious edges from phrases like "not related to" or "avoid using".

## Timestamp Protection

Timestamps are tool-owned and protected:

- **Preserve policy**: Trust existing timestamps on read
- **Protect policy**: Ignore model-written timestamps on import/update
- **Protected fields**: `created_at`, `updated_at`, `expires_at`
- **Provenance**: `source.kind` tracks who wrote the section (cli, hermes, sync, planner)

## Hermes Integration

Hermes commands adapt RBMEM for agent workflows:

- **Load**: Output JSON with sections, resolved content, graph, timeline
- **Save**: Apply append/replace updates with provenance tracking
- **Plan**: Run SAT planning and store results in RBMEM
- **Doctor**: Verify memory health and load paths

`hermes:memory` sections enforce append-only writes for safety.

## HTTP Server

The Axum server (`src/server/`) exposes RBMEM as a REST API:

```
GET  /health
POST /memories
GET  /memories/:name
PUT  /memories/:name
DELETE /memories/:name
GET  /memories/:name/sections/:path
PUT  /memories/:name/sections/:path
DELETE /memories/:name/sections/:path
POST /memories/:name/query
POST /memories/:name/context
POST /memories/:name/diff
POST /memories/:name/merge
POST /memories/:name/export
```

## Performance Characteristics

| Operation | Complexity | Measured (46 sections) |
|-----------|------------|------------------------|
| Parse document | O(n) | 85µs (16K chars) |
| Query (indexed + IDF) | O(k log n) | 38µs/query |
| Query (no index) | O(n) | Linear scan fallback |
| Graph traversal | O(d × e) | <1µs (depth=2) |
| Index build | O(n) | 259µs |
| Health check | O(n) | Single pass through sections |
| Conflict detection | O(n) | HashMap grouping |
| DPLL solve | O(2^n) worst | VSIDS improves average case |

## Testing Strategy

- **Unit tests** (29): Parser, document model, commands, graph inference
- **Integration tests** (5): CLI workflows, library API, concurrent operations
- **Regression tests** (3): Parser edge cases in `tests/parser_regression.rs`
- **Encryption tests** (30): Edge cases, concurrent encryption, wrong-key failures
- **Concurrent tests** (10): Multi-threaded reads, writes, queries, graph operations
- **Planner tests** (3): Constraint satisfaction, stress benchmarks
- **Server tests** (1): Health endpoint
- **MCP server tests** (178): Python test suite in `mcp-server/`
- **Benchmarks**: RBMEM vs Markdown comparison, planner stress tests
- **Total: 285 tests** across Rust and Python

## Future Work

- Vector embeddings for semantic search
- Distributed sync protocol
- WebAssembly build for browser agents
- Python/TypeScript bindings
- Publish to crates.io
- Semantic section similarity (beyond keyword matching)
- Adaptive query expansion based on result quality
- Incremental parsing for large documents

# Architecture

`rbmem` is a single-crate Rust CLI with three primary modules:

| Module | Responsibility |
| --- | --- |
| `src/parser.rs` | Forgiving Nom parser for RBMEM v1.3 syntax, including canonical and human-friendly section delimiters. |
| `src/document.rs` | Core data model, smart merge, timestamp protection, provenance, graph view, compact rendering, relation inference, and validation helpers. |
| `src/main.rs` | CLI command wiring, Markdown conversion, sync workflow, context query/pack assembly, review/diff commands, Hermes integration, and file I/O. |

## Data Flow

1. Commands read RBMEM or Markdown input from disk.
2. Markdown input is converted into RBMEM sections with dotted paths derived from heading hierarchy.
3. RBMEM input is parsed with a timestamp policy:
   - `Preserve` for trusted reads.
   - `Protect` for imports and updates where model-written timestamps must be ignored.
4. The document model renders the requested view:
   - Full RBMEM for auditability.
   - Compact or minified RBMEM for context windows.
   - JSON for Hermes and automation.
   - DOT or JSON graph output.
   - Task-specific context selected by query text, named packs, parent paths, and graph neighbors.
5. Updates write the full durable RBMEM document back to disk.

## Context Assembly

`rbmem query` and `rbmem context` select sections by path/content matches, can include parent sections for resolved inheritance, and can pull graph neighbors by depth. `rbmem pack` makes that repeatable through `.rbmempacks` files with `include`, `query`, `graph_depth`, and `mode` fields.

## Provenance And Review

Sections may include an optional `source` block with `kind`, `path`, and `actor`.
Markdown sync stamps synced sections with `kind: "markdown"`, CLI updates use `kind: "cli"`, and Hermes saves use `kind: "hermes"` unless a payload supplies a more specific source. `rbmem review` combines parser/validation warnings with provenance and inferred-edge review hints, while `rbmem diff` compares section-level changes between two RBMEM files.

## Graph Model

RBMEM graph output combines:

- Implicit `contains` edges from dotted section paths.
- Manual relations written in a section's `graph.relations`.
- Inferred relations added by `convert-from-md --infer-relations` or `rbmem infer`.

Inference is configurable with `--inference-strategy off|explicit|balanced|aggressive`. `balanced` preserves the default heuristic, `explicit` accepts only direct relation phrases, and `aggressive` lowers the effective confidence threshold for recall-heavy agent indexing. Markdown sync can set the same behavior in `.rbmemsync` with `inference_strategy: explicit`.

Graph JSON marks each edge source so agents can distinguish trusted manual edges from inferred ones.

## Timestamp Protection

Timestamps are tool-owned. The parser can preserve trusted timestamps when reading an existing file, but import/update flows use protected timestamps so generated content cannot silently forge `created_at`, `updated_at`, or `expires_at`.

## Hermes Integration

Hermes commands are a thin adapter over the core document model. They expose a stable JSON shape with sections, resolved content, graph edges, timeline entries, and context renderings. `hermes:memory` sections use safer append behavior for agent-written memory.

## Phase 2 Diagnostics

`rbmem doctor` reports the CLI release version, locked RBMEM document format version, optional file existence, parser status, validation status, section count, and graph edge count.

`rbmem hermes doctor` extends that check for Hermes memory files by verifying the Hermes JSON/context load path. When passed `--rbmem-cli`, it also runs that configured binary with `--version`, which helps catch Windows/WSL path mismatches and stale release binaries before a chat request times out.

## Phases 3 And 4 Tool Output

Phase 3 adds JSON diagnostics to `rbmem doctor` and `rbmem hermes doctor` through `--format json`. The JSON payloads are intended for agents and automation, while text remains the default for humans.

Phase 4 adds JSON context assembly to `rbmem query`, `rbmem context`, and `rbmem pack`. These commands now expose the rendered context string, selected sections, source document metadata, options, and graph view under the `rbmem.context.v1` schema.

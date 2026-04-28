# Architecture

`rbmem` is a single-crate Rust CLI with three primary modules:

| Module | Responsibility |
| --- | --- |
| `src/parser.rs` | Forgiving Nom parser for RBMEM v1.3 syntax, including canonical and human-friendly section delimiters. |
| `src/document.rs` | Core data model, smart merge, timestamp protection, graph view, compact rendering, relation inference, and validation helpers. |
| `src/main.rs` | CLI command wiring, Markdown conversion, sync workflow, Hermes integration, and file I/O. |

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
5. Updates write the full durable RBMEM document back to disk.

## Graph Model

RBMEM graph output combines:

- Implicit `contains` edges from dotted section paths.
- Manual relations written in a section's `graph.relations`.
- Inferred relations added by `convert-from-md --infer-relations` or `rbmem infer`.

Graph JSON marks each edge source so agents can distinguish trusted manual edges from inferred ones.

## Timestamp Protection

Timestamps are tool-owned. The parser can preserve trusted timestamps when reading an existing file, but import/update flows use protected timestamps so generated content cannot silently forge `created_at`, `updated_at`, or `expires_at`.

## Hermes Integration

Hermes commands are a thin adapter over the core document model. They expose a stable JSON shape with sections, resolved content, graph edges, timeline entries, and context renderings. `hermes:memory` sections use safer append behavior for agent-written memory.

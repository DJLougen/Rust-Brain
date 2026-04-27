# AIF

[![CI](https://github.com/basbe/aif/actions/workflows/ci.yml/badge.svg)](https://github.com/basbe/aif/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

`aif` is a Rust CLI for Agent Interchange Format (`.aif`) v1.3: a structured, temporal, graph-aware memory format for LLM and agent workflows.

Markdown is excellent for humans, but agents need more than headings and prose. AIF keeps human-editable content while adding stable section addresses, protected timestamps, merge rules, graph edges, timeline views, compact context output, and Hermes Harness JSON integration.

## Why AIF Instead Of Plain Markdown?

Agents fail most often when context is ambiguous, stale, or hard to update safely. AIF gives every memory item a path such as `architecture.api.auth`, protects timestamps so models cannot forge them, and exposes both implicit hierarchy edges and explicit/inferred relations. The same file can be rendered as full audit-friendly AIF, compact agent context, minified context, DOT graphs, trees, timelines, or Hermes JSON.

Use Markdown when the content is only for people. Use AIF when an agent needs to read, resolve, update, merge, prune, or reason over the content repeatedly.

## Quick Start

```powershell
cargo install --path .
aif create memory.aif
aif read memory.aif
aif update memory.aif --section goals --type list --content "- Ship the project"
aif read memory.aif --resolve --minified
```

Convert Markdown into structured AIF:

```powershell
aif convert-from-md examples\sample.md examples\from_md.aif --infer-relations
aif graph examples\from_md.aif --format json
aif tree examples\from_md.aif
```

Sync a Markdown folder for agent consumption:

```powershell
aif sync C:\notes C:\agent-memory --infer-relations --min-confidence 0.7
aif sync C:\notes C:\agent-memory --watch --infer-relations
```

## Core Commands

| Command | Purpose |
| --- | --- |
| `create <file.aif>` | Create a new AIF document. |
| `read <file.aif> [--resolve] [--compact] [--minified]` | Read raw or resolved content for humans or agents. |
| `update <file.aif> --section <path>` | Safely update a section with protected timestamps. |
| `convert-from-md <in.md> <out.aif>` | Preserve Markdown heading hierarchy as dotted AIF paths. |
| `infer <file.aif>` | Add inferred graph relations without overwriting manual ones. |
| `sync <md-folder> <out-folder>` | Convert a folder of Markdown files into enriched AIF. |
| `graph <file.aif> --format json\|dot` | Export implicit, manual, and inferred relations. |
| `tree <file.aif>` | Show hierarchy. |
| `timeline <file.aif>` | Show temporal ordering. |
| `validate <file.aif>` | Parse and validate the document. |
| `hermes ...` | Load, save, watch, and scaffold Hermes-friendly memory. |

## AIF vs Markdown: Real Measurements

Token counts are approximate word counts multiplied by 1.3 from local CLI comparisons. Raw AIF is larger because it carries temporal and graph metadata; compact and minified output are intended for LLM context windows.

| Sample | Markdown Raw | AIF Raw | AIF Compact | AIF Minified | Sections | Graph Edges |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `examples/sample.md` | 265 bytes / 52 tokens | 1,289 bytes / 228 tokens | 473 bytes / 87 tokens | 312 bytes | 3 | 3 |
| Obsidian `index` | 3,339 bytes / 491 tokens | 9,888 bytes / 1,529 tokens | 737 tokens | n/a | 21 | 53 |
| Obsidian `concept_ior` | 4,077 bytes / 647 tokens | 6,748 bytes / 1,093 tokens | 758 tokens | n/a | 12 | 18 |
| Obsidian `comparison_ior_facilitation` | 3,261 bytes / 465 tokens | 6,589 bytes / 1,013 tokens | 603 tokens | n/a | 15 | 25 |
| Obsidian `source_posner_cohen` | 2,547 bytes / 369 tokens | 5,362 bytes / 800 tokens | 458 tokens | n/a | 9 | 15 |

| Capability | Markdown | AIF |
| --- | --- | --- |
| Stable addressable sections | Heading text only | Dotted paths such as `agents.reader.capabilities` |
| Inheritance and resolved views | Manual convention | Built-in smart merge |
| Protected timestamps | Easy for models to alter | Tool-owned `created_at`, `updated_at`, `expires_at` |
| Graph relationships | Links or prose | Implicit hierarchy plus manual and inferred edges |
| Agent-safe updates | Rewrite-prone | Section-level append, replace, and Hermes memory behavior |
| LLM context size | Naturally concise | Use `--compact` or `--minified` for near-Markdown context |
| Machine validation | Weak | Parser warnings, validation, graph/tree/timeline views |

## Recommended Agent Workflow

1. Humans write or edit Markdown notes, project rules, and memory.
2. Run `aif sync notes aif-memory --infer-relations` to produce structured `.aif`.
3. Agents load context with `aif read memory.aif --resolve --minified` or `aif hermes load memory.aif --resolve --minified`.
4. Agents update only named sections with `aif update` or `aif hermes save`.
5. Periodically inspect `aif graph`, `aif tree`, `aif timeline`, and `aif validate`.

For long-running projects, keep full AIF as the durable source of truth and feed compact or minified resolved views into the context window.

## Hermes Harness

AIF includes first-class Hermes commands:

```powershell
aif hermes init my-agent-memory
aif hermes load my-agent-memory.aif --resolve --compact
aif hermes load my-agent-memory.aif --resolve --minified
aif hermes save my-agent-memory.aif --json '{"sections":[{"path":"memory","type":"hermes:memory","content":"- User prefers concise answers."}]}'
aif hermes watch my-agent-memory.aif
```

Existing read commands can also emit Hermes-preferred JSON:

```powershell
aif read examples\sample.aif --resolve --hermes
aif read examples\sample.aif --resolve --hermes-inject --minified
```

See [HERMES.md](HERMES.md) for the recommended Hermes agent instructions and payload shape.

## Development

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --all-features
```

This repository uses Graphify for local code graph notes. After code changes, refresh with:

```powershell
graphify-rs build --path . --output graphify-out --no-llm --code-only --format report,wiki,json
```

## Status

AIF v1.3 is intentionally small and single-crate. The format and CLI are ready for private GitHub use, local agent memory workflows, and iterative experimentation before publishing to crates.io.

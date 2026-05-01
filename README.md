# RBMEM

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

`rbmem` is a Rust command-line tool for **Rust-Brain Memory Format** (`.rbmem`) v1.3.

RBMEM is a small, readable file format for agent memory, instructions, research notes, project rules, and long-lived context. It keeps the parts humans like about Markdown, but adds the structure agents need: stable section paths, protected timestamps, hierarchy, graph relationships, timelines, validation, and compact context output.

This repository is intended for private agent workflows first. It is useful when you want humans to keep writing notes, while agents consume a cleaner, safer, machine-readable memory layer.

## What Problem Does This Solve?

Markdown is easy to write, but it is loose:

- headings are not stable identifiers
- timestamps can be invented or overwritten by a model
- hierarchy and inheritance are only conventions
- graph relationships are hidden in prose or links
- updating one memory item often means rewriting the whole file
- agents have to spend tokens guessing what matters

RBMEM turns the same material into explicit sections such as:

```text
agents.reader.capabilities
project.rules.testing
memory.user.preferences
architecture.backend.graph
```

Agents can then read, resolve, update, prune, search, graph, and summarize memory without treating a document as one undifferentiated blob.

## When To Use RBMEM

Use RBMEM when:

- an agent needs durable memory across sessions
- instructions need stable section names
- you want tool-owned timestamps
- you want parent/child inheritance
- you want graph relationships between ideas, modules, or notes
- you want compact LLM context without losing full metadata on disk
- you want Hermes agents to load and update memory safely

Use plain Markdown when:

- the content is only for humans
- you do not need timestamps, graph edges, validation, or safe updates
- you want the smallest possible hand-authored note

## Quick Start

Build the CLI:

```powershell
cd C:\Users\basbe\Desktop\Rust-Brain
cargo build --release
```

Create a memory file:

```powershell
.\target\release\rbmem.exe create memory.rbmem
```

Add a section:

```powershell
.\target\release\rbmem.exe update memory.rbmem --section goals --type list --content "- Ship the project"
```

Read it back:

```powershell
.\target\release\rbmem.exe read memory.rbmem
```

Feed the smallest useful resolved view to an agent:

```powershell
.\target\release\rbmem.exe read memory.rbmem --resolve --minified
```

Ask RBMEM for task-specific context:

```powershell
.\target\release\rbmem.exe query memory.rbmem "github code review" --resolve --minified --graph-depth 1
.\target\release\rbmem.exe context memory.rbmem --task "review this PR" --resolve --minified
```

## RBMEM At A Glance

An RBMEM file is plain text:

```rbmem
rbmem# RBMEM v1.3 - Rust-Brain Memory Format

meta:
  version: 1.3
  purpose: "personal-agent-memory"
  generated_at: "2026-04-27T13:10:00Z"
  last_updated: "2026-04-27T13:10:00Z"
  valid_until: null
  created_by: "me"
  default_expiry_days: null
  compact_mode: minified

[SECTION: project.rules]
type: list
temporal:
  created_at: "2026-04-27T13:10:00Z"
  updated_at: "2026-04-27T13:10:00Z"
  expires_at: null
content: |
  - Prefer small, tested changes.
  - Preserve user intent.
[END SECTION]
```

The CLI protects timestamps during import/update flows, so model-generated timestamps do not silently become trusted history.

## Markdown Conversion

Convert one Markdown file:

```powershell
.\target\release\rbmem.exe convert-from-md examples\sample.md examples\from_md.rbmem --infer-relations
```

Markdown headings become dotted paths:

```text
# Agents
## Reader
### Capabilities
```

becomes:

```text
agents
agents.reader
agents.reader.capabilities
```

Sync a whole Markdown folder:

```powershell
.\target\release\rbmem.exe sync C:\notes C:\agent-memory --infer-relations --min-confidence 0.7
```

Watch for changes:

```powershell
.\target\release\rbmem.exe sync C:\notes C:\agent-memory --watch --infer-relations
```

## Hermes Harness Workflow

Create a Hermes memory file:

```powershell
.\target\release\rbmem.exe hermes init my-agent-memory
```

Load Hermes-optimized JSON:

```powershell
.\target\release\rbmem.exe hermes load my-agent-memory.rbmem --resolve --minified
```

Save agent memory safely:

```powershell
.\target\release\rbmem.exe hermes save my-agent-memory.rbmem --json '{"sections":[{"path":"memory","type":"hermes:memory","content":"- User prefers concise engineering answers.","mode":"append"}]}'
```

Store GEPA/Hermes self-evolution artifacts in the same `.rbmem` file:

```powershell
.\target\release\rbmem.exe hermes save my-agent-memory.rbmem --json '{"sections":[{"path":"evolution.skills.github-code-review.history","type":"hermes:memory","content":"- GEPA candidate held for human review.","mode":"append"},{"path":"evolution.skills.github-code-review.candidates.demo-gepa-001.metadata","type":"json","content":"{\"status\":\"needs_human_review\",\"candidate_score\":0.82,\"auto_applied\":false}","mode":"replace"}]}'
```

Print a ready-to-paste Hermes context block:

```powershell
.\target\release\rbmem.exe read my-agent-memory.rbmem --resolve --hermes-inject --minified
```

## Context Packs And Review

Create `.rbmempacks` next to a memory file:

```text
[pack: code_review]
include:
  - rules
  - memory.user.preferences
query: "pull request tests"
graph_depth: 1
mode: minified
```

Load that named context pack:

```powershell
.\target\release\rbmem.exe pack memory.rbmem code_review --resolve
```

Review and compare memory changes:

```powershell
.\target\release\rbmem.exe review memory.rbmem
.\target\release\rbmem.exe diff before.rbmem after.rbmem
```

Sections can now carry optional provenance:

```rbmem
source:
  kind: "markdown"
  path: "notes/project.md"
  actor: "sync"
```

Current local Hermes integration uses:

```text
C:\Users\basbe\.hermes\MEMORY.rbmem
```

Hermes Workspace is configured to load that file through:

```powershell
C:\Users\basbe\Desktop\Rust-Brain\target\release\rbmem.exe hermes load C:\Users\basbe\.hermes\MEMORY.rbmem --resolve --minified
```

See [HERMES.md](HERMES.md) for agent instructions and the save payload shape. See [docs/HERMES_SELF_EVOLUTION.md](docs/HERMES_SELF_EVOLUTION.md) and [examples/hermes-self-evolution.rbmem](examples/hermes-self-evolution.rbmem) for the RBMEM-backed GEPA self-evolution workflow.

## Command Cheat Sheet

| Command | What It Does |
| --- | --- |
| `create <file.rbmem>` | Create a new RBMEM document. |
| `read <file.rbmem>` | Read a document. |
| `read <file.rbmem> --resolve` | Apply hierarchy merge rules before rendering. |
| `read <file.rbmem> --resolve --compact` | Hide most metadata for shorter context. |
| `read <file.rbmem> --resolve --minified` | Smallest agent-oriented text view. |
| `update <file.rbmem> --section <path>` | Safely add or update one section. |
| `convert-from-md <in.md> <out.rbmem>` | Convert Markdown headings into dotted RBMEM paths. |
| `sync <md-folder> <out-folder>` | Convert a Markdown folder into RBMEM files. |
| `infer <file.rbmem>` | Infer graph relations from prose. |
| `query <file.rbmem> <text>` | Return matching task-specific context. |
| `context <file.rbmem> --task <text>` | Alias for task-oriented context assembly. |
| `pack <file.rbmem> <name>` | Render a named context pack from `.rbmempacks`. |
| `diff <before.rbmem> <after.rbmem>` | Report section-level memory changes. |
| `review <file.rbmem>` | Validate and flag agent-written or inferred memory for human review. |
| `graph <file.rbmem> --format json` | Export graph nodes and edges. |
| `graph <file.rbmem> --format dot` | Export a DOT graph. |
| `tree <file.rbmem>` | Show section hierarchy. |
| `timeline <file.rbmem>` | Show temporal entries. |
| `validate <file.rbmem>` | Validate parser compatibility. |
| `hermes load <file.rbmem>` | Output Hermes-friendly JSON. |
| `hermes save <file.rbmem> --json <payload>` | Apply Hermes-style memory updates. |
| `hermes watch <file.rbmem>` | Watch a file and print Hermes JSON on changes. |

## RBMEM vs Markdown: Real Measurements

Token counts are approximate word counts multiplied by 1.3 from local CLI comparisons. Raw RBMEM is larger because it stores metadata; compact and minified output are the intended context-window views.

| Sample | Markdown Raw | RBMEM Raw | RBMEM Compact | RBMEM Minified | Sections | Graph Edges |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `examples/sample.md` | 265 bytes / 52 tokens | 1,289 bytes / 228 tokens | 473 bytes / 87 tokens | 312 bytes | 3 | 3 |
| Obsidian `index` | 3,339 bytes / 491 tokens | 9,888 bytes / 1,529 tokens | 737 tokens | n/a | 21 | 53 |
| Obsidian `concept_ior` | 4,077 bytes / 647 tokens | 6,748 bytes / 1,093 tokens | 758 tokens | n/a | 12 | 18 |
| Obsidian `comparison_ior_facilitation` | 3,261 bytes / 465 tokens | 6,589 bytes / 1,013 tokens | 603 tokens | n/a | 15 | 25 |
| Obsidian `source_posner_cohen` | 2,547 bytes / 369 tokens | 5,362 bytes / 800 tokens | 458 tokens | n/a | 9 | 15 |

| Capability | Markdown | RBMEM |
| --- | --- | --- |
| Stable section IDs | Heading text only | Dotted paths |
| Hierarchy | Visual convention | Parent/child paths plus resolved merge |
| Timestamps | Untrusted text | Tool-protected temporal fields |
| Graph relationships | Links/prose | Implicit, manual, and inferred edges |
| Agent updates | Rewrite-prone | Section-level append/replace |
| Context size | Naturally concise | `--compact` and `--minified` views |
| Validation | Weak | Parser, warnings, tree, graph, timeline |

## Development

Run the normal Rust checks:

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --all-features
```

Package check:

```powershell
cargo package --allow-dirty
```

Refresh the local Graphify code graph after code changes:

```powershell
graphify-rs build --path . --output graphify-out --no-llm --code-only --format report,wiki,json
```

## Repository Status

This is a private, single-crate Rust CLI for personal agent memory workflows. It is ready for local use and private GitHub iteration. The crate is not published to crates.io yet.

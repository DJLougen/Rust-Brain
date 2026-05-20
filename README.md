# RBMEM

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Rust-Brain is the project. RBMEM is the structured memory format. `rbmem` is the CLI and Rust library for reading, writing, validating, and serving `.rbmem` files.

`rbmem` currently targets **Rust-Brain Memory Format** (`.rbmem`) v1.4.0, while still parsing v1.3 files for migration and compatibility.

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

```bash
./target/release/rbmem create memory.rbmem
```

Add a section:

```powershell
.\target\release\rbmem.exe update memory.rbmem --section goals --type list --content "- Ship the project"
```

```bash
./target/release/rbmem update memory.rbmem --section goals --type list --content "- Ship the project"
```

Read it back:

```powershell
.\target\release\rbmem.exe read memory.rbmem
```

```bash
./target/release/rbmem read memory.rbmem
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

```bash
./target/release/rbmem query memory.rbmem "github code review" --resolve --minified --graph-depth 1
./target/release/rbmem context memory.rbmem --task "review this PR" --resolve --minified
```

## SAT Planning

Rust-Brain now includes advanced SAT planning as a first-class RBMEM feature. `rbmem plan` loads goals, tasks, rules, constraints, preferences, graph context, and context sections from `.rbmem` files, encodes the planning problem as CNF, solves it with Kissat/CaDiCaL when available, falls back to a native Rust DPLL solver, and stores the resulting plan back into memory with timestamps and graph relations.

Plan from an explicit goal:

```powershell
.\target\release\rbmem.exe plan "deploy agent release" --file examples\sat-planning.rbmem
```

Derive the goal from memory:

```powershell
.\target\release\rbmem.exe plan --from-memory --file examples\sat-planning.rbmem
```

Use solver/proof options:

```powershell
.\target\release\rbmem.exe plan "deploy agent release" --file examples\sat-planning.rbmem --solver kissat --proof --verify-proof
.\target\release\rbmem.exe plan "deploy agent release" --file examples\sat-planning.rbmem --cube-and-conquer --format json
```

Plan with a stored context pack:

```powershell
.\target\release\rbmem.exe plan "deploy agent release" --file examples\sat-planning.rbmem --pack release_ops
```

The planner writes sections under `plans.<goal>.<timestamp>.*`:

```text
plans.deploy-agent-release-20260518200000.goal
plans.deploy-agent-release-20260518200000.steps
plans.deploy-agent-release-20260518200000.sat
plans.deploy-agent-release-20260518200000.proof
timeline
```

Rules can be plain RBMEM list items:

```text
- deploy agent release requires run focused regression tests
- publish release notes requires deploy agent release
- gather requirements conflicts with deploy agent release
- avoid deploying without validation
```

DRAT support is proof-aware: external proof-producing solvers and `drat-trim` are used when installed; the native solver records the DIMACS/model for SAT plans and verifies simple empty-clause UNSAT proofs internally.

## RBMEM At A Glance

An RBMEM file is plain text:

```rbmem
rbmem# RBMEM v1.4.0 - Rust-Brain Memory Format

meta:
  version: 1.4.0
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
.\target\release\rbmem.exe convert-from-md examples\sample.md examples\from_md.rbmem --infer-relations --inference-strategy balanced
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
.\target\release\rbmem.exe sync C:\notes C:\agent-memory --infer-relations --min-confidence 0.7 --inference-strategy explicit
```

Watch for changes:

```powershell
.\target\release\rbmem.exe sync C:\notes C:\agent-memory --watch --infer-relations
```

Infer graph relations on an existing file:

```powershell
.\target\release\rbmem.exe infer memory.rbmem --inference-strategy aggressive --min-confidence 0.7
```

Inference strategies are `off`, `explicit`, `balanced`, and `aggressive`. `balanced` is the default and preserves the original heuristic; `explicit` only accepts prose with relation verbs such as "uses" or "depends on"; `aggressive` lowers the effective threshold for broader recall.

## Hermes Harness Workflow

Create a Hermes memory file:

```powershell
.\target\release\rbmem.exe hermes init my-agent-memory
```

Load Hermes-optimized JSON:

```powershell
.\target\release\rbmem.exe hermes load my-agent-memory.rbmem --resolve --minified
```

Plan from Hermes memory with the native SAT planner:

```powershell
.\target\release\rbmem.exe hermes plan my-agent-memory.rbmem --goal "deploy agent release" --format json
.\target\release\rbmem.exe hermes plan my-agent-memory.rbmem --from-memory --pack release_ops --format json
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

Run Phase 2 diagnostics:

```powershell
.\target\release\rbmem.exe doctor memory.rbmem
.\target\release\rbmem.exe hermes doctor my-agent-memory.rbmem --rbmem-cli .\target\release\rbmem.exe
```

Ask for machine-readable diagnostics and context:

```powershell
.\target\release\rbmem.exe doctor memory.rbmem --format json
.\target\release\rbmem.exe query memory.rbmem "github code review" --resolve --minified --graph-depth 1 --format json
.\target\release\rbmem.exe pack memory.rbmem code_review --resolve --format json
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
| `read <file.rbmem> --decrypt` | Include encrypted sections after resolving the encryption key. |
| `update <file.rbmem> --section <path>` | Safely add or update one section. |
| `update <file.rbmem> --section <path> --dry-run` | Preview an update without writing it. |
| `delete-section <file.rbmem> --section <path>` | Remove one section by path. |
| `prune <file.rbmem>` | Remove expired sections. |
| `encrypt <file.rbmem> --section <path>` | Encrypt one section with AES-256-GCM. |
| `decrypt <file.rbmem> --section <path>` | Persistently decrypt one encrypted section. |
| `convert-from-md <in.md> <out.rbmem>` | Convert Markdown headings into dotted RBMEM paths. |
| `sync <md-folder> <out-folder>` | Convert a Markdown folder into RBMEM files. |
| `infer <file.rbmem>` | Infer graph relations from prose. |
| `query <file.rbmem> <text>` | Return matching task-specific context; use `--format json` or `--decrypt` when needed. |
| `context <file.rbmem> --task <text>` | Alias for task-oriented context assembly; use `--format json` or `--decrypt` when needed. |
| `plan "<goal>" [--file <file.rbmem>] [--pack <name>]` | Build a SAT planning problem from RBMEM memory, solve it, and store the resulting plan back into memory. |
| `plan --from-memory [--file <file.rbmem>]` | Derive the goal from `goals`/`tasks` sections before planning. |
| `pack <file.rbmem> <name>` | Render a named context pack from `.rbmempacks`; use `--format json` for tool callers. |
| `diff <before.rbmem> <after.rbmem> --format text|json|yaml` | Report typed section-level memory changes. |
| `merge <base.rbmem> <local.rbmem> <remote.rbmem> --strategy manual` | Run a three-way section merge. |
| `migrate <file.rbmem> --dry-run` | Explicitly normalize older RBMEM documents and preserve `_source_version`. |
| `review <file.rbmem>` | Validate and flag agent-written or inferred memory for human review. |
| `doctor [file.rbmem]` | Report CLI version, RBMEM format version, parse status, validation status, section count, and graph edges; supports `--format json`. |
| `--log-format json` | Emit structured `tracing` logs; pair with `RUST_LOG=info`. |
| `graph <file.rbmem> --format json` | Export graph nodes and edges. |
| `graph <file.rbmem> --format dot` | Export a DOT graph. |
| `export <file.rbmem> --format dot|mermaid|cytoscape|gexf` | Export graph visualizations for Graphviz, Markdown, Cytoscape, or Gephi. |
| `serve --bind localhost:3000 --dir <memory_dir>` | Run the Axum REST API server. |
| `tree <file.rbmem>` | Show section hierarchy. |
| `timeline <file.rbmem>` | Show temporal entries. |
| `validate <file.rbmem>` | Validate parser compatibility. |
| `hermes load <file.rbmem>` | Output Hermes-friendly JSON. |
| `hermes plan <file.rbmem> --goal <goal>` | Run SAT planning as a Hermes-native memory operation and store the plan in RBMEM. |
| `hermes save <file.rbmem> --json <payload>` | Apply Hermes-style memory updates. |
| `hermes save <file.rbmem> --json-file payload.json` | Apply Hermes updates from a file. |
| `hermes doctor <file.rbmem>` | Check RBMEM memory health and verify Hermes JSON/context loading; supports `--format json`. |
| `hermes watch <file.rbmem>` | Watch a file and print Hermes JSON on changes. |

## Encryption

Encrypted sections use AES-256-GCM and are stored as `type: encrypted` with `nonce`, `ciphertext`, and `encrypted_at`. Normal reads and queries skip encrypted sections. Add `--decrypt` when the caller should resolve the key and include decrypted content.

Key lookup order is:

1. `RBMEM_ENCRYPTION_KEY`
2. `~/.rbmem/key`
3. interactive prompt

The key must be 32 raw bytes or base64-encoded 32 bytes.

```powershell
$env:RBMEM_ENCRYPTION_KEY = "<base64-encoded-32-byte-key>"
rbmem encrypt memory.rbmem --section secrets.api
rbmem read memory.rbmem
rbmem read memory.rbmem --decrypt
rbmem query memory.rbmem "api token" --decrypt
rbmem decrypt memory.rbmem --section secrets.api
```

## Diff, Merge, Export, And Server

Typed diffs can render as text, JSON, or YAML:

```powershell
rbmem diff base.rbmem changed.rbmem --format json
```

Three-way merge compares `base`, `local`, and `remote` at section granularity. `manual` creates `type: conflict` sections when both sides changed the same section.

```powershell
rbmem merge base.rbmem local.rbmem remote.rbmem --strategy manual --output merged.rbmem
```

Graph exports support DOT, Mermaid, Cytoscape JSON, and GEXF:

```powershell
rbmem export memory.rbmem --format mermaid
rbmem export memory.rbmem --format gexf
```

The HTTP server exposes health, memory CRUD, section CRUD, query, context, diff, merge, and export routes:

```powershell
rbmem serve --bind localhost:3000 --dir .\memories
```

## Rust Library API

Rust-Brain now builds both a `rbmem` binary and a `rbmem` library crate. The CLI remains the supported human-facing interface, while Rust callers can use the same create, read, update, query, context, load, save, and diff behavior without spawning a process.

```rust
use chrono::Utc;
use rbmem::{
    create, query, update, ContextOptions, CreateOptions, OutputFormat, SectionType,
    TimestampPolicy, UpdateOptions,
};

# fn demo() -> Result<(), rbmem::RbmemError> {
let file = "memory.rbmem";
let now = Utc::now();

create(
    file,
    CreateOptions {
        created_by: "agent".to_string(),
        purpose: "personal-agent-memory".to_string(),
        default_expiry_days: None,
        human: false,
        now,
    },
)?;

update(
    file,
    UpdateOptions {
        section: "agents.reader".to_string(),
        section_type: SectionType::Text,
        content: "Reads memory carefully.".to_string(),
        human: false,
        dry_run: false,
        now,
    },
)?;

let context = query(
    file,
    "reader",
    ContextOptions {
        resolve: true,
        compact: false,
        minified: true,
        graph_depth: 0,
        decrypt: false,
        key: None,
        format: OutputFormat::Text,
        policy: TimestampPolicy::Preserve,
    },
)?;
# Ok(())
# }
```

Library functions return `Result<T, RbmemError>`. New writes use RBMEM v1.4.0, while v1.3 files still parse and normalize with `_source_version`.

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

### Benchmark Results (Automated)

The following benchmark compares RBMEM against plain Markdown across 5 dimensions using a 46-section knowledge base and 15 realistic queries:

![RBMEM vs Markdown Benchmark](benchmark_results.png)

**Key findings:**

| Metric | RBMEM | Markdown | Result |
| --- | ---: | ---: | --- |
| Avg Precision | 11.5% | 6.4% (full dump) | **1.8× better** |
| Avg Recall | 95.6% | — | Targeted retrieval |
| Graph-Aware Recall | 100% | N/A | **Unique capability** |
| Token Savings (query) | 41.9% | baseline | Smarter context |
| Temporal Awareness | per-section | none | Staleness detection |
| Compact Modes | 3 modes | none | Flexible output |
| Encryption | per-section AES-256 | none | Security |
| Provenance Tracking | source + version + hash | none | Auditability |

Run the benchmark yourself:

```powershell
cargo bench --bench rbmem_vs_markdown -- --nocapture
```

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

# Hermes + RBMEM

Use `.rbmem` as the durable memory and instruction layer for Hermes agents. Humans can keep writing Markdown, then run `rbmem sync`; agents should read and write RBMEM through the Hermes commands.

## Recommended Workflow

```powershell
rbmem hermes init my-project
rbmem hermes load my-project.rbmem --resolve --minified
rbmem hermes save my-project.rbmem --json '{"sections":[{"path":"memory","type":"hermes:memory","content":"- User prefers terse context."}]}'
rbmem hermes save my-project.rbmem --json-file payload.json
```

For Markdown folders:

```powershell
rbmem sync notes RBMEM-memory --infer-relations --min-confidence 0.7
rbmem sync notes RBMEM-memory --watch --infer-relations
```

## Hermes Agent Instructions

- Load memory before planning:
  `rbmem hermes load project.rbmem --resolve --compact`
- Use SAT planning for constrained task plans:
  `rbmem hermes plan project.rbmem --goal "<goal>" --format json`
- Add `--pack <name>` when the task should be planned against a stored context pack.
- Use `rbmem hermes plan project.rbmem --from-memory --format json` when the active goal is already stored in `goals` or `tasks`.
- Treat `sections[].path` as the stable memory address.
- Prefer `hermes:memory` for append-only facts, preferences, and observations.
- `hermes:memory` sections are append-only; use `mode: "auto"` or `mode: "append"`.
- Use `mode: "replace"` only for non-`hermes:memory` sections when correcting stale content.
- Never invent timestamps. The tool protects timestamps.
- Read `graph.edges` for explicit, inferred, and implicit relationships.

## Hermes JSON Save Shape

```json
{
  "sections": [
    {
      "path": "memory",
      "type": "hermes:memory",
      "content": "- The user wants minified context by default.",
      "mode": "auto"
    },
    {
      "path": "tasks",
      "type": "list",
      "content": "- Verify graph output",
      "mode": "append"
    }
  ]
}
```

`mode` can be `auto`, `append`, or `replace`. For `hermes:memory`, `auto` appends safely and avoids duplicate exact entries.

## Self-Evolution Memory Schema

GEPA/Hermes self-evolution should write candidates and evidence into RBMEM, not
sidecar Markdown files. Use `evolution.*` paths so every mutation is durable,
loadable, and reviewable:

```text
evolution.runs.<run_id>.config
evolution.runs.<run_id>.report
evolution.runs.<run_id>.latest_trace
evolution.runs.<run_id>.traces.<example_id>
evolution.skills.<skill_name>.history
evolution.skills.<skill_name>.candidates.<run_id>.skill
evolution.skills.<skill_name>.candidates.<run_id>.diff
evolution.skills.<skill_name>.candidates.<run_id>.metadata
```

Example save payload:

```json
{
  "sections": [
    {
      "path": "evolution.runs.demo-gepa-001.report",
      "type": "text",
      "content": "PR-ready GEPA report with score deltas, safety gates, and review notes.",
      "mode": "replace"
    },
    {
      "path": "evolution.skills.github-code-review.history",
      "type": "hermes:memory",
      "content": "- 2026-04-29: Candidate demo-gepa-001 improved validation score and is waiting for human review.",
      "mode": "append"
    },
    {
      "path": "evolution.skills.github-code-review.candidates.demo-gepa-001.skill",
      "type": "text",
      "content": "Candidate skill text goes here.",
      "mode": "replace"
    },
    {
      "path": "evolution.skills.github-code-review.candidates.demo-gepa-001.metadata",
      "type": "json",
      "content": "{\"status\":\"needs_human_review\",\"baseline_score\":0.71,\"candidate_score\":0.82,\"auto_applied\":false}",
      "mode": "replace"
    }
  ]
}
```

Self-evolution must not auto-commit or silently replace live skills. Store the
candidate, diff, report, traces, and metadata first; apply the candidate only
after regression tests and human review pass.

## Injection Block

```powershell
rbmem read project.rbmem --resolve --hermes-inject --minified
```

This prints a ready-to-paste JSON context block for Hermes prompts.

## Modern RBMEM Capabilities for Hermes

Hermes should prefer the highest-signal RBMEM operation for the job instead of always loading the whole memory file. Use these rules when planning, retrieving context, writing memory, and reviewing changes.

### Context Retrieval

- Use `rbmem query` or `rbmem context` for task-specific retrieval before loading full memory.
- Use `--resolve` when parent sections contain rules or defaults that child sections inherit.
- Use `--minified` for model context unless the task requires timestamps, source, graph, or audit metadata.
- Use `--graph-depth 1` when the task may depend on related sections; increase only when the first result is insufficient.
- Use `--format json` when another tool, script, or agent step will parse the output.

Recommended commands:

```powershell
rbmem query project.rbmem "current task or error message" --resolve --minified --graph-depth 1
rbmem context project.rbmem --task "implement this change safely" --resolve --minified --graph-depth 1 --format json
```

### Encrypted Sections

Encrypted sections are skipped by normal reads and queries. Hermes must not assume missing secret sections are absent; they may be intentionally encrypted.

- Use encrypted sections for credentials, private user details, API tokens, security notes, and sensitive tool configuration.
- Do not decrypt unless the current task explicitly needs that content.
- Prefer `RBMEM_ENCRYPTION_KEY` or `~/.rbmem/key` for non-interactive sessions.
- Never write decrypted secrets into logs, reports, diffs, traces, or `hermes:memory`.

Recommended commands:

```powershell
rbmem encrypt project.rbmem --section secrets.api
rbmem read project.rbmem --resolve --minified --decrypt
rbmem query project.rbmem "api credential needed for this run" --resolve --minified --decrypt
rbmem decrypt project.rbmem --section secrets.api
```

### Safer Change Review

Before and after a material memory update, Hermes should preserve a comparable copy and review the section-level delta.

Recommended workflow:

```powershell
Copy-Item project.rbmem project.before.rbmem
rbmem hermes save project.rbmem --json '{"sections":[{"path":"memory","type":"hermes:memory","content":"- New durable fact.","mode":"append"}]}'
rbmem diff project.before.rbmem project.rbmem --format json
rbmem review project.rbmem
rbmem doctor project.rbmem --format json
```

For branch or multi-agent conflicts, use three-way merge. The `manual` strategy creates `type: conflict` sections instead of silently choosing a side.

```powershell
rbmem merge base.rbmem local.rbmem remote.rbmem --strategy manual --output merged.rbmem
rbmem review merged.rbmem
```

Hermes should treat `type: conflict` sections as blocking review items. Resolve them by replacing the conflict section with the intended final section content.

### Graph-Aware Reasoning

Hermes should inspect graph edges when a task involves dependencies, categories, candidate evolution, tool relationships, or project architecture.

Use graph export for visual review:

```powershell
rbmem export project.rbmem --format mermaid
rbmem export project.rbmem --format cytoscape
rbmem export project.rbmem --format gexf
```

Use Mermaid output in Markdown reports when humans need to inspect the memory graph. Use Cytoscape or GEXF when an external graph tool will analyze relationships.

### Server Mode

For long-running Hermes sessions, start the RBMEM server once and prefer HTTP calls from runtime tools instead of repeatedly shelling out.

```powershell
rbmem serve --bind localhost:3000 --dir .\.hermes
```

Server routes include:

```text
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

Use CLI commands for human-operated workflows and one-off tasks. Use server mode for agent runtimes, repeated queries, local tools, and RBForge integration.

### Harness Decision Rules

- For planning: use `rbmem plan` when rules, constraints, prerequisites, or conflicts matter; use `query` or `context` for lightweight retrieval.
- For sensitive data: encrypt the section, retrieve with `--decrypt` only for the exact task, and never persist decrypted material elsewhere.
- For updates: write with `hermes save`, then run `diff`, `review`, and `doctor`.
- For conflicting memory: run `merge --strategy manual` and treat conflict sections as requiring human or explicit agent resolution.
- For relationship-heavy tasks: use graph depth during retrieval and `export --format mermaid` for review artifacts.
- For repeated agent/tool access: start `serve` and use HTTP endpoints.

### Updated Default Hermes Load Pattern

Use the smallest useful context by default:

```powershell
rbmem context project.rbmem --task "<current task>" --resolve --minified --graph-depth 1 --format json
```

When the task needs a concrete feasible plan, ask the SAT planner to write the plan back into RBMEM:

```powershell
rbmem hermes plan project.rbmem --goal "<current goal>" --solver auto --format json
rbmem hermes plan project.rbmem --from-memory --cube-and-conquer --format json
rbmem hermes plan project.rbmem --goal "<current goal>" --pack code_review --format json
```

Fall back to the full Hermes JSON view when the agent needs complete memory state:

```powershell
rbmem hermes load project.rbmem --resolve --minified
```

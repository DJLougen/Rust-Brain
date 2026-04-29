# Hermes + RBMEM

Use `.rbmem` as the durable memory and instruction layer for Hermes agents. Humans can keep writing Markdown, then run `rbmem sync`; agents should read and write RBMEM through the Hermes commands.

## Recommended Workflow

```powershell
rbmem hermes init my-project
rbmem hermes load my-project.rbmem --resolve --minified
rbmem hermes save my-project.rbmem --json '{"sections":[{"path":"memory","type":"hermes:memory","content":"- User prefers terse context."}]}'
```

For Markdown folders:

```powershell
rbmem sync notes RBMEM-memory --infer-relations --min-confidence 0.7
rbmem sync notes RBMEM-memory --watch --infer-relations
```

## Hermes Agent Instructions

- Load memory before planning:
  `rbmem hermes load project.rbmem --resolve --compact`
- Treat `sections[].path` as the stable memory address.
- Prefer `hermes:memory` for append-only facts, preferences, and observations.
- Use `mode: "replace"` only when correcting stale content.
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

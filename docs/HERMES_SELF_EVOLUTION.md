# Hermes Self-Evolution with RBMEM

Rust-Brain can store GEPA/Hermes self-evolution runs directly inside `.rbmem`
instead of writing loose Markdown artifacts. The Hermes optimizer should use
`rbmem hermes save` for every candidate, report, trace, and review note so the
full optimization history remains addressable, timestamped, validated, and
loadable by future Hermes sessions.

## Section Layout

Use stable paths under `evolution.*`:

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

Recommended section types:

| Path suffix | Type | Purpose |
| --- | --- | --- |
| `.config` | `json` | Optimizer settings, model choices, dataset source, budgets, safety gates. |
| `.report` | `text` | PR-ready justification, Pareto summary, safety result, and review checklist. |
| `.latest_trace` | `json` | Compact pointer to the most recent or most useful trace. |
| `.traces.*` | `json` | Rich execution trace with task, reasoning summary, tool calls, errors, metrics, and feedback. |
| `.history` | `hermes:memory` | Append-only evolution notes for a skill. |
| `.skill` | `text` | Candidate evolved skill instructions or prompt text. |
| `.diff` | `text` | Unified diff or human-readable replacement summary. |
| `.metadata` | `json` | Scores, objective values, semantic similarity, regression status, and review status. |

## Save Payload

Hermes can write a self-evolution bundle with the existing save command:

```powershell
rbmem hermes save .hermes/MEMORY.rbmem --json '{
  "sections": [
    {
      "path": "evolution.runs.20260429T194521Z-github-code-review.config",
      "type": "json",
      "mode": "replace",
      "content": "{\"optimizer\":\"gepa\",\"max_metric_calls\":120,\"dataset_source\":\"golden+sessions\"}"
    },
    {
      "path": "evolution.runs.20260429T194521Z-github-code-review.report",
      "type": "text",
      "mode": "replace",
      "content": "GEPA run summary, Pareto result, safety gates, and review notes."
    },
    {
      "path": "evolution.skills.github-code-review.history",
      "type": "hermes:memory",
      "mode": "append",
      "content": "- 2026-04-29: GEPA candidate produced but held for review."
    },
    {
      "path": "evolution.skills.github-code-review.candidates.20260429T194521Z-github-code-review.skill",
      "type": "text",
      "mode": "replace",
      "content": "Candidate SKILL.md text goes here."
    },
    {
      "path": "evolution.skills.github-code-review.candidates.20260429T194521Z-github-code-review.diff",
      "type": "text",
      "mode": "replace",
      "content": "--- current\\n+++ candidate\\n@@ ..."
    },
    {
      "path": "evolution.skills.github-code-review.candidates.20260429T194521Z-github-code-review.metadata",
      "type": "json",
      "mode": "replace",
      "content": "{\"status\":\"needs_human_review\",\"score\":0.82,\"baseline_score\":0.71,\"semantic_similarity\":0.63}"
    }
  ]
}'
```

For large traces, save one trace section per evaluation example. Keep
`latest_trace` small so Hermes can quickly discover where the detailed evidence
lives.

## Loading for a Future Run

Before a new evolution pass, load the memory with resolved context:

```powershell
rbmem hermes load .hermes/MEMORY.rbmem --resolve --minified
```

The optimizer should inspect:

- `evolution.skills.<skill_name>.history` for prior decisions.
- `evolution.skills.<skill_name>.candidates.*.metadata` for previous scores and safety failures.
- `evolution.runs.*.report` for human review outcomes.
- `graph.edges` for relationships between runs, skills, candidate artifacts, and reports.

## Safety Policy

Self-evolution writes candidates, not automatic source changes. A candidate may
be copied into a live skill only after:

- Regression tests pass.
- Character and token budgets are within limits.
- Semantic similarity is above the configured threshold, unless a human accepts
  a deliberate rewrite.
- The report explains every material mutation using trace evidence.
- A human reviews the RBMEM candidate, diff, and metadata sections.

See `examples/hermes-self-evolution.rbmem` for a complete valid fixture.

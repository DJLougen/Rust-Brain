# Hermes + AIF

Use `.aif` as the durable memory and instruction layer for Hermes agents. Humans can keep writing Markdown, then run `aif sync`; agents should read and write AIF through the Hermes commands.

## Recommended Workflow

```powershell
aif hermes init my-project
aif hermes load my-project.aif --resolve --minified
aif hermes save my-project.aif --json '{"sections":[{"path":"memory","type":"hermes:memory","content":"- User prefers terse context."}]}'
```

For Markdown folders:

```powershell
aif sync notes aif-memory --infer-relations --min-confidence 0.7
aif sync notes aif-memory --watch --infer-relations
```

## Hermes Agent Instructions

- Load memory before planning:
  `aif hermes load project.aif --resolve --compact`
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

## Injection Block

```powershell
aif read project.aif --resolve --hermes-inject --minified
```

This prints a ready-to-paste JSON context block for Hermes prompts.

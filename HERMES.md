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

## Injection Block

```powershell
rbmem read project.rbmem --resolve --hermes-inject --minified
```

This prints a ready-to-paste JSON context block for Hermes prompts.

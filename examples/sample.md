# Agents

Durable personal memory needs a format that can be repaired after LLM output.

## Reader

The reader validates incoming memory and uses Writer before memory is merged.

## Writer

The writer emits canonical RBMEM after timestamps are protected by the tool.

# RBForge Agent Setup Brief

The canonical agent setup artifact is
[AGENT_SETUP.rbmem](AGENT_SETUP.rbmem). This Markdown file is a human-readable
mirror. Agents that can load Rust-Brain memory should use the `.rbmem` file.

Point an agent at this file when you want it to set up or use RBForge without
reading the whole repository first.

## Purpose

RBForge lets an agent create durable runtime tools. A good agent should use it
only when a reusable capability is missing and likely to be useful again.

RBForge stores tools in RBMEM through the Rust-Brain `rbmem` CLI.

- Rust-Brain repo: `https://github.com/DJLougen/Rust-Brain`
- RBForge: `mcp-server/` directory within Rust-Brain

## Setup Steps

1. Install RBForge:

```shell
python -m pip install -e .[dev]
```

2. Make Rust-Brain `rbmem` available:

```shell
export RBMEM_CLI=/path/to/rbmem
```

3. Set Python import path when running from source:

```shell
export PYTHONPATH=src
```

4. Verify:

```shell
python -m compileall -q src tests examples scripts
pytest -q
```

5. Optional Hermes bridge:

```shell
python scripts/install_hermes_bridge.py
```

The bridge uses `$HERMES_HOME`, `$HERMES_RBMEM`, and `$RBMEM_CLI` when set. It
does not require hardcoded user-specific paths.

## Runtime Tool Policy

Forge a tool only when all of these are true:

- The missing capability is reusable.
- The implementation can be small and deterministic.
- Inputs can be described by a strict JSON object schema.
- Output can be a JSON-serializable dictionary.
- The tool does not need secrets or credentials.
- The tool does not require unsafe filesystem, shell, memory, or network access.

Do not forge a tool for one-off work.

## How To Forge

Use `forge_tool`:

```python
from RBForge import forge_tool

result = forge_tool(
    name="count_tracebacks",
    description="Count Python tracebacks in a supplied log string.",
    schema={
        "type": "object",
        "properties": {"log": {"type": "string", "default": "Traceback"}},
        "required": ["log"],
    },
    implementation=(
        "def run(log: str) -> dict:\n"
        "    return {'traceback_count': log.count('Traceback')}\n"
    ),
    category="debugger",
    expected_output_keys=["traceback_count"],
    memory_path="memory.rbmem",
)
```

Inspect `result["status"]`.

- `registered`: call it with `run_forged_tool`.
- `review_queued`: do not run it automatically.
- `sandbox_failed`: inspect validation output and fix the implementation.
- `validation_failed`: fix the schema, name, category, or source.

## How To Reuse

Use `run_forged_tool`:

```python
from RBForge import run_forged_tool

result = run_forged_tool(
    name="count_tracebacks",
    arguments={"log": "Traceback\nboom"},
    memory_path="memory.rbmem",
)
```

Read `result["ok"]`, `result["result"]`, `result["error"]`, and
`result["metrics"]`.

## Categories

Use low-impact categories for normal tools:

- `analysis`
- `debugger`
- `profiler`
- `refactor`
- `report`

High-impact categories require review:

- `filesystem`
- `memory`
- `shell`
- `web_bubble`

Do not bypass review for high-impact tools unless a human explicitly approves
the integration policy.

## Required Tool Shape

Implementation rules:

- Python only for executable tools in this release.
- Define `def run(...) -> dict`.
- Return JSON-serializable data.
- Avoid hidden global state.
- Avoid dynamic code execution.
- Avoid broad imports.

Schema rules:

- Top-level schema must be an object.
- Include `properties`.
- Include `required` when inputs are mandatory.
- Provide defaults or `expected_args` so validation has realistic sample inputs.

## Agent Decision Loop

Use this sequence:

1. Search the registry for an existing suitable tool.
2. If none exists, decide whether the gap is reusable.
3. If reusable and safe, call `forge_tool`.
4. If status is `registered`, call `run_forged_tool`.
5. If status is `review_queued`, report that review is needed.
6. If validation fails, fix the proposal once or abandon the forge.
7. Record the result and prefer reuse next time.

## Minimal Agent Instruction

Use this as a system or developer instruction:

```text
Use RBForge for reusable missing capabilities. Before forging, prefer existing
tools in tools.registry. When forging, provide a complete Python run(...)
implementation, a strict JSON object schema, a category, expected output keys,
and representative sample arguments if defaults are insufficient. Run a forged
tool only when forge_tool returns status=registered. Treat filesystem, memory,
shell, and web_bubble categories as review-required. Never forge tools that
handle secrets, credentials, arbitrary shell execution, destructive file writes,
or one-off work.
```

## Quick Health Check

Run:

```shell
python -m compileall -q src tests examples scripts
pytest -q
```

Then test a simple forge against a temporary memory file. If that works, RBForge
is ready for agent use.

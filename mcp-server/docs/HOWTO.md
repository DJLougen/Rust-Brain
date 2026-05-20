# RBForge How-To Guide

This guide is for someone who has never used RBForge before. It explains what
RBForge is, why it exists, how to install it, how to create tools, how to reuse
them, and how to wire it into an agent.

## Mental Model

RBForge gives an agent a way to create durable tools while it is working.

Without RBForge, an agent might repeatedly write throwaway snippets like:

- Count failures in this log.
- Extract TODOs from this source dump.
- Summarize dependency edges.
- Normalize a messy report.
- Rank suspicious stack traces.

Those snippets disappear after the task. With RBForge, the agent can turn a
snippet into a named tool, validate it, save it to RBMEM, and call it again
later.

For debugging, this means the agent can learn to use or forge compact helpers
for traceback triage, failing-test clustering, suspect-file extraction, and
lock-contention summaries instead of rereading raw logs every time.

RBForge depends on the RBMEM format and CLI from
[Rust-Brain](https://github.com/DJLougen/Rust-Brain). RBMEM is the durable
memory layer. RBForge is the tool-forging layer that writes useful tool records
into that memory.

## Core Terms

- `forge_tool`: creates, validates, tests, persists, and registers a new tool.
- `run_forged_tool`: runs a tool that was previously registered.
- `.rbmem`: Rust-Brain memory file.
- `tools.custom.{name}`: where the full forged tool record is stored.
- `tools.registry`: the index agents use to discover existing tools.
- `category`: a label that controls validation behavior, review policy, and
  import permissions.
- `schema`: JSON Schema describing arguments the tool accepts.
- `implementation`: Python source code for the tool. It must expose `run(...)`
  or a function matching the tool name.

## Installation

Create an environment and install RBForge:

```shell
git clone https://github.com/DJLougen/Rust-Brain.git
cd Rust-Brain/mcp-server
python -m venv .venv
. .venv/bin/activate
python -m pip install -e .[dev]
```

On Windows PowerShell:

```powershell
git clone https://github.com/DJLougen/Rust-Brain.git
cd Rust-Brain/mcp-server
python -m venv .venv
.\.venv\Scripts\Activate.ps1
python -m pip install -e .[dev]
```

Install or build `rbmem` from
[Rust-Brain](https://github.com/DJLougen/Rust-Brain), then make it available on
`PATH` or set `RBMEM_CLI`:

```shell
export RBMEM_CLI=/path/to/rbmem
```

PowerShell:

```powershell
$env:RBMEM_CLI = "C:\path\to\rbmem.exe"
```

If `rbmem` is not found, RBForge can clone and build Rust-Brain automatically
when it needs the CLI, assuming Git and Cargo are installed.

For best results, use Rust-Brain / RBMEM `v0.4.0` or newer. RBForge uses its
JSON diagnostics and JSON context assembly commands.

## Verify Setup

Run:

```shell
export PYTHONPATH=src
python -m compileall -q src tests examples scripts
pytest -q
```

You can also check RBMEM integration directly:

```shell
rbforge doctor memory.rbmem
rbforge doctor memory.rbmem --format json
rbforge eval debugger
```

```python
from rbforge_core.rbmem import RbmemStore

store = RbmemStore("memory.rbmem")
print(store.rbmem_version())
print(store.doctor()["hermes_load"]["status"])
```

PowerShell:

```powershell
$env:PYTHONPATH = "src"
python -m compileall -q src tests examples scripts
pytest -q
```

Run the mini demo:

```shell
python scripts/demo_invention_loop.py
```

You should see a simulated invention loop that creates a tool, saves it, and
uses it.

## First Tool

This example creates a `count_tracebacks` tool and saves it to `memory.rbmem`.

```python
from RBForge import forge_tool, run_forged_tool

result = forge_tool(
    name="count_tracebacks",
    description="Count Python tracebacks in a supplied log string.",
    schema={
        "type": "object",
        "properties": {
            "log": {
                "type": "string",
                "default": "Traceback\nValueError: example",
            }
        },
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

print(result["status"])
print(result["section_path"])

run_result = run_forged_tool(
    name="count_tracebacks",
    arguments={"log": "ok\nTraceback\nboom\nTraceback\nagain"},
    memory_path="memory.rbmem",
)

print(run_result["result"])
```

Expected output shape:

```python
registered
tools.custom.count_tracebacks
{"traceback_count": 2}
```

## Anatomy Of A Tool Proposal

A tool proposal has these core fields:

```python
forge_tool(
    name="short_snake_case_name",
    description="Human-readable purpose, at least a sentence.",
    schema={...},
    implementation="def run(...) -> dict:\n    ...\n",
    category="analysis",
    dependencies=["tools.builtin.search_files"],
    expected_args={...},
    expected_output_keys=["key_one", "key_two"],
    memory_path="memory.rbmem",
)
```

Use `expected_args` when the schema defaults are not enough to generate a good
test case. Use `expected_output_keys` when the tool should always return
specific dictionary keys.

## Good Tool Design

Good forged tools are:

- Small: one job, easy to validate.
- Deterministic: same inputs produce the same outputs.
- Structured: return JSON-serializable dictionaries.
- Reusable: useful across more than one task.
- Safe: no unnecessary shell, filesystem, network, or process access.

Poor forged tools are:

- Vague wrappers around a whole workflow.
- Tools that require hidden global state.
- Tools that scrape arbitrary URLs without review.
- Tools that read or write files when text arguments would work.
- Tools that duplicate an existing built-in tool.

## Example: Rank Log Errors

```python
from RBForge import forge_tool, run_forged_tool

forge_tool(
    name="rank_error_lines",
    description="Rank repeated error lines in a log by frequency.",
    schema={
        "type": "object",
        "properties": {
            "log": {
                "type": "string",
                "default": "ERROR db\nINFO ok\nERROR db\nERROR api",
            }
        },
        "required": ["log"],
    },
    implementation=(
        "from collections import Counter\n\n"
        "def run(log: str) -> dict:\n"
        "    errors = [line.strip() for line in log.splitlines() if 'ERROR' in line]\n"
        "    ranked = Counter(errors).most_common()\n"
        "    return {'error_count': len(errors), 'ranked': ranked}\n"
    ),
    category="debugger",
    expected_output_keys=["error_count", "ranked"],
    memory_path="memory.rbmem",
)

result = run_forged_tool(
    name="rank_error_lines",
    arguments={"log": "ERROR db\nERROR api\nERROR db\nINFO ok"},
    memory_path="memory.rbmem",
)

print(result["result"])
```

## Example: Extract TODOs

```python
from RBForge import forge_tool, run_forged_tool

forge_tool(
    name="extract_todos",
    description="Extract TODO and FIXME comments from supplied text.",
    schema={
        "type": "object",
        "properties": {
            "text": {
                "type": "string",
                "default": "TODO: add tests\nprint('done')",
            }
        },
        "required": ["text"],
    },
    implementation=(
        "def run(text: str) -> dict:\n"
        "    items = []\n"
        "    for line_no, line in enumerate(text.splitlines(), start=1):\n"
        "        upper = line.upper()\n"
        "        if 'TODO' in upper or 'FIXME' in upper:\n"
        "            items.append({'line': line_no, 'text': line.strip()})\n"
        "    return {'count': len(items), 'items': items}\n"
    ),
    category="analysis",
    expected_output_keys=["count", "items"],
    memory_path="memory.rbmem",
)

result = run_forged_tool(
    name="extract_todos",
    arguments={"text": "TODO: wire CLI\nok\nFIXME: handle no results"},
    memory_path="memory.rbmem",
)

print(result["result"])
```

## Review Queue Behavior

RBForge treats these categories as high-impact:

- `filesystem`
- `memory`
- `shell`
- `web_bubble`

These tools can pass validation but still land in a review queue instead of
being registered immediately. This is intentional. It prevents an agent from
silently activating tools that can affect the filesystem, shell, memory, or web
surface.

Example:

```python
from RBForge import forge_tool

result = forge_tool(
    name="shell_echo_probe",
    description="Prepare a constrained shell category tool for review.",
    schema={"type": "object", "properties": {}, "required": []},
    implementation=(
        "import subprocess\n\n"
        "def run() -> dict:\n"
        "    return {'module': subprocess.__name__}\n"
    ),
    category="shell",
    memory_path="memory.rbmem",
)

print(result["status"])
```

Expected status:

```text
review_queued
```

## Inspecting The RBMEM File

After forging a tool, inspect the memory file with Rust-Brain:

```shell
rbforge doctor memory.rbmem
rbmem validate memory.rbmem
rbmem read memory.rbmem tools.registry
rbmem read memory.rbmem tools.custom.count_tracebacks
```

Exact `rbmem` subcommands may vary as Rust-Brain evolves. Use:

```shell
rbmem --help
rbmem read --help
```

Rust-Brain lives at
[https://github.com/DJLougen/Rust-Brain](https://github.com/DJLougen/Rust-Brain).

## Hermes Setup

RBForge includes a bridge installer for Hermes-style local agent harnesses:

```shell
export PYTHONPATH=src
python scripts/install_hermes_bridge.py
```

The installer uses:

- `$HERMES_HOME/config.yaml`, or `~/.hermes/config.yaml`
- `$HERMES_RBMEM`, or `~/.hermes/MEMORY.rbmem`
- `$RBMEM_CLI`, or an `rbmem` binary found on `PATH`

It adds the `RBForge` toolset and writes RBMEM instructions under:

- `tools.RBForge.autonomy`
- `tools.RBForge.bridge`

Then start Hermes with:

```shell
hermes -s RBForge
```

When Hermes detects a reusable missing capability, it should call `forge_tool`.
If the returned status is `registered`, it can immediately call
`run_forged_tool`.

## RL Debugger Signal

The training config rewards debugger use when the model:

- calls a debugger before patching,
- extracts a root cause from debugger output,
- reuses an existing debugger tool when one exists,
- forges a reusable debugger only when the missing capability is real.

It penalizes skipping an available debugger, ignoring debugger output, or
forging duplicate low-value debugging helpers.

Run the local debugger eval to make the signal concrete:

```shell
rbforge eval debugger
```

The output is intentionally compact:

```text
debugger-use-rate: 100.0%
root-cause-hit-rate: 100.0%
baseline-root-cause-hit-rate: 40.0%
avg-turn-reduction: 44.3%
estimated-turns-saved: 47
reusable-debuggers-created: 9
```

## Agent Prompt Pattern

Use this instruction with an agent:

```text
When you identify a reusable missing capability, call forge_tool with a complete
Python implementation, a strict JSON schema, a category, and expected output
keys. If forge_tool returns status=registered, call run_forged_tool with the
task arguments. Do not forge tools for one-off work, unsafe filesystem writes,
secret handling, credential access, or arbitrary shell execution.
```

For a dedicated agent setup file, use the RBMEM-native
[Agent Setup Memory](AGENT_SETUP.rbmem). The Markdown
[Agent Setup Brief](AGENT_SETUP.md) is provided for human review, but agents
that understand Rust-Brain should load the `.rbmem` file.

## Troubleshooting

`rbmem CLI not found`:

- Install Rust-Brain and put `rbmem` on `PATH`.
- Or set `RBMEM_CLI=/path/to/rbmem`.

`validation_failed`:

- Check that the tool name is snake_case and at least three characters.
- Check that the JSON schema is an object schema.
- Check that the implementation defines `run(...)`.
- Avoid forbidden imports and calls.

`sandbox_failed`:

- Run the generated implementation locally with the sample arguments.
- Add `expected_args` if schema defaults do not produce a useful test.
- Keep output JSON-serializable.

`review_queued`:

- The category is high-impact or `review_required=True`.
- Review the saved candidate before activating it.

`run_forged_tool` cannot find the tool:

- Confirm the same `memory_path` is used for forging and running.
- Inspect `tools.registry` in the `.rbmem` file.
- Confirm the forge result status was `registered`.

## Safety Checklist

Before registering a new tool, ask:

- Is the tool reusable?
- Is the schema strict enough?
- Does it avoid credentials and secrets?
- Does it avoid broad filesystem, shell, or network access?
- Does it return a dictionary with predictable keys?
- Can it pass generated tests with representative sample arguments?

If the answer is no, do not register it automatically.

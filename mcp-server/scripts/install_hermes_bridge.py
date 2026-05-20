"""Install RBForge into a local Hermes configuration.

Example:
    python scripts/install_hermes_bridge.py

The script updates the Hermes config under ``$HERMES_HOME`` or ``~/.hermes``.
When run on Windows with WSL available, it also updates the WSL Hermes config.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from pathlib import Path

import yaml

PROJECT_ROOT = Path(__file__).resolve().parents[1]
HERMES_HOME = Path(os.environ.get("HERMES_HOME", Path.home() / ".hermes")).expanduser()
HERMES_RBMEM = Path(os.environ.get("HERMES_RBMEM", HERMES_HOME / "MEMORY.rbmem")).expanduser()
TRACE_PATH = PROJECT_ROOT / "data" / "traces" / "hermes_RBForge.jsonl"
LEGACY_NAME = "tool" + "forge"


def main() -> None:
    """Install the bridge and print a JSON status summary."""
    changed = []
    config_path = HERMES_HOME / "config.yaml"
    if config_path.exists():
        update_yaml_config(config_path)
        changed.append(str(config_path))
    rbmem_result = update_rbmem()
    if rbmem_result["ok"]:
        changed.append(str(HERMES_RBMEM))
    wsl_result = update_wsl_config()
    print(
        json.dumps(
            {"ok": True, "changed": changed, "rbmem": rbmem_result, "wsl": wsl_result},
            indent=2,
        )
    )


def update_yaml_config(path: Path) -> None:
    config = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    toolsets = config.setdefault("toolsets", [])
    toolsets[:] = [toolset for toolset in toolsets if toolset != LEGACY_NAME]
    for toolset in ["hermes-cli", "web", "RBForge"]:
        if toolset not in toolsets:
            toolsets.append(toolset)
    config.pop(LEGACY_NAME, None)
    config.setdefault("RBForge", {})
    config["RBForge"].update(bridge_config(find_rbmem_cli()))
    path.write_text(yaml.safe_dump(config, sort_keys=False), encoding="utf-8")


def bridge_config(rbmem_cli: str) -> dict[str, object]:
    return {
        "enabled": True,
        "project_root": str(PROJECT_ROOT),
        "memory_path": str(HERMES_RBMEM),
        "rbmem_cli": rbmem_cli,
        "trace_path": str(TRACE_PATH),
        "autonomous": True,
    }


def update_rbmem() -> dict[str, object]:
    rbmem = find_rbmem_cli()
    if not rbmem:
        return {"ok": False, "reason": "rbmem CLI not found"}
    payload = {
        "sections": [
            {
                "path": "tools.RBForge.autonomy",
                "type": "hermes:memory",
                "mode": "replace",
                "content": "\n".join(autonomy_lines()),
            },
            {
                "path": "tools.RBForge.bridge",
                "type": "json",
                "mode": "replace",
                "content": json.dumps(
                    {
                        "toolset": "RBForge",
                        "tools": ["forge_tool", "run_forged_tool"],
                        "project_root": str(PROJECT_ROOT),
                        "memory_path": str(HERMES_RBMEM),
                        "rbmem_cli": rbmem,
                        "trace_path": str(TRACE_PATH),
                    },
                    sort_keys=True,
                ),
            },
        ]
    }
    subprocess.run(
        [rbmem, "hermes", "save", str(HERMES_RBMEM), "--json", json.dumps(payload)],
        check=True,
    )
    remove_legacy_rbmem_sections(HERMES_RBMEM)
    return {"ok": True, "path": str(HERMES_RBMEM)}


def autonomy_lines() -> list[str]:
    return [
        "- RBForge is enabled for autonomous runtime tool creation.",
        (
            "- When a missing reusable capability is blocking progress, call "
            "forge_tool from inside <think> with a complete Python implementation."
        ),
        (
            "- Prefer forging composable analysis/refactor/debug/profiler tools "
            "with minimal dependencies."
        ),
        (
            "- After forge_tool returns status=registered, immediately call "
            "run_forged_tool with the new tool name and task arguments."
        ),
        (
            "- Forged tools persist under tools.custom.{name}; registry lives at "
            "tools.registry; RBMEM timestamps are tool-owned."
        ),
        (
            "- High-impact categories such as filesystem, memory, shell, and "
            "web_bubble enter review unless explicitly allowed."
        ),
    ]


def find_rbmem_cli() -> str:
    env_path = os.environ.get("RBMEM_CLI")
    candidates = [env_path, shutil.which("rbmem"), shutil.which("rbmem.exe")]
    existing_config = HERMES_HOME / "config.yaml"
    if existing_config.exists():
        config = yaml.safe_load(existing_config.read_text(encoding="utf-8")) or {}
        memory = config.get("memory", {})
        rbforge = config.get("RBForge", {})
        candidates.extend([memory.get("rbmem_cli_path"), rbforge.get("rbmem_cli")])
    for candidate in candidates:
        if candidate and Path(candidate).expanduser().exists():
            return str(Path(candidate).expanduser())
    return ""


def remove_legacy_rbmem_sections(path: Path) -> None:
    if not path.exists():
        return
    text = path.read_text(encoding="utf-8")
    section = re.escape("tools." + LEGACY_NAME)
    text = re.sub(
        rf"\n?\[SECTION: {section}\.[^\]]+\]\n.*?\[END SECTION\]\n?",
        "\n",
        text,
        flags=re.DOTALL,
    )
    path.write_text(text, encoding="utf-8")


def update_wsl_config() -> dict[str, object]:
    if os.name != "nt" or not shutil.which("wsl"):
        return {"ok": False, "reason": "wsl unavailable"}
    project_root = windows_to_wsl_path(PROJECT_ROOT)
    memory_path = windows_to_wsl_path(HERMES_RBMEM)
    trace_path = windows_to_wsl_path(TRACE_PATH)
    rbmem_cli = wsl_rbmem_cli(find_rbmem_cli())
    wsl_native_rbmem = subprocess.run(
        ["wsl", "--", "sh", "-lc", "command -v rbmem || true"],
        capture_output=True,
        text=True,
        check=False,
    )
    if wsl_native_rbmem.stdout.strip():
        rbmem_cli = wsl_native_rbmem.stdout.strip()
    script = r'''
import json
import os
import sys
from pathlib import Path

rbmem_cli, project_root, memory_path, trace_path, legacy_name = sys.argv[1:6]
hermes_home = Path(os.environ.get("HERMES_HOME", Path.home() / ".hermes")).expanduser()
path = hermes_home / "config.yaml"
if not path.exists():
    print(json.dumps({"ok": False, "reason": "missing Hermes WSL config"}))
    raise SystemExit(0)
text = path.read_text()

def remove_top_level_block(source: str, key: str) -> str:
    lines = source.splitlines()
    output = []
    skipping = False
    for line in lines:
        if line == f"{key}:":
            skipping = True
            continue
        if skipping and line and not line[0].isspace() and not line.startswith("-"):
            skipping = False
        if not skipping:
            output.append(line)
    return "\n".join(output).rstrip() + "\n"

text = text.replace(f"- {legacy_name}", "- RBForge")
if "toolsets:" not in text:
    text += "\ntoolsets:\n"
if "- RBForge" not in text:
    text = text.replace("toolsets:\n", "toolsets:\n- RBForge\n", 1)
text = remove_top_level_block(text, legacy_name)
text = remove_top_level_block(text, "RBForge")
text += f"""
RBForge:
  enabled: true
  project_root: {project_root}
  memory_path: {memory_path}
  rbmem_cli: {rbmem_cli}
  trace_path: {trace_path}
  autonomous: true
"""
path.write_text(text)
print(json.dumps({"ok": True, "path": str(path)}))
'''
    completed = subprocess.run(
        [
            "wsl",
            "--",
            "python3",
            "-c",
            script,
            rbmem_cli,
            project_root,
            memory_path,
            trace_path,
            LEGACY_NAME,
        ],
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        return {"ok": False, "error": completed.stderr.decode("utf-8", errors="replace").strip()}
    try:
        stdout = completed.stdout.decode("utf-8", errors="replace")
        return json.loads(stdout.strip().splitlines()[-1])
    except Exception:
        return {"ok": True, "raw": completed.stdout.decode("utf-8", errors="replace").strip()}


def windows_to_wsl_path(path: Path) -> str:
    completed = subprocess.run(
        ["wsl", "--", "wslpath", "-a", str(path).replace("\\", "/")],
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode == 0:
        return completed.stdout.strip()
    return str(path)


def wsl_rbmem_cli(rbmem_cli: str) -> str:
    if not rbmem_cli:
        return "rbmem"
    return windows_to_wsl_path(Path(rbmem_cli))


if __name__ == "__main__":
    main()

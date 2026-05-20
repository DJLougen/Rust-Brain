"""RBForge health and usage diagnostics."""

from __future__ import annotations

import argparse
import json
from collections.abc import Sequence
from pathlib import Path
from typing import Any, Protocol

from rbforge_core import __version__
from rbforge_core.compactor import compact_memory, identify_stale_sections
from rbforge_core.health import compute_health_score
from rbforge_core.rbmem import RbmemStore
from rbforge_core.review import get_pending_reviews
from rbforge_core.temporal import trend_analysis, usage_pattern
from rbforge_core.version import check_rbmem_compatibility
from rbforge_core.versioning import list_snapshots, create_snapshot, diff_snapshots, auto_snapshot_on_forged


class DoctorStore(Protocol):
    memory_path: Path | str

    def rbmem_version(self) -> str: ...

    def doctor(self) -> dict[str, Any]: ...

    def read_registry(self) -> list[dict[str, Any]]: ...

    def context(
        self,
        query: str,
        *,
        resolve: bool = True,
        minified: bool = True,
        graph_depth: int = 1,
    ) -> dict[str, Any]: ...


def add_doctor_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "memory_path",
        nargs="?",
        default="memory.rbmem",
        help="RBMEM file to inspect.",
    )
    parser.add_argument(
        "--rbmem-cli",
        default=None,
        help="Path to rbmem or rbmem.exe. Defaults to RBMEM_CLI or PATH.",
    )
    parser.add_argument(
        "--format",
        choices=("text", "json"),
        default="text",
        help="Output format.",
    )


def build_report(
    store: DoctorStore,
    *,
    rbforge_version: str = __version__,
) -> dict[str, Any]:
    """Collect RBForge, RBMEM, memory, registry, and forged-tool metrics."""
    memory_path = Path(store.memory_path)
    existed_before_check = memory_path.exists()
    rbmem_version = store.rbmem_version()
    compatibility = check_rbmem_compatibility(rbmem_version)
    doctor_payload = store.doctor()
    exists_after_check = memory_path.exists()
    registry = store.read_registry()
    tool_records = _read_tool_records(store, registry)
    validated_tools = sum(1 for record in tool_records if _is_validated(record))
    success_rates = list(_success_rates(tool_records))
    average_success_rate = (
        round(sum(success_rates) / len(success_rates), 4) if success_rates else None
    )
    category_metrics = _category_metrics(tool_records)
    debugger_records = [record for record in tool_records if _is_debugger_tool(record)]
    debugger_rates = list(_success_rates(debugger_records))
    forged_tools = len(tool_records)
    validation_rate = round(validated_tools / forged_tools, 4) if forged_tools else None

    # Detect declining sections using usage_pattern
    declining_sections = _detect_declining_sections(store)

    return {
        "schema": "rbforge.doctor.v1",
        "rbforge_version": rbforge_version,
        "rbmem_cli_version": rbmem_version,
        "rbmem_compatibility": compatibility.__dict__,
        "memory_path": str(memory_path),
        "memory_file": {
            "exists_before_check": existed_before_check,
            "exists": exists_after_check,
            "size_bytes": memory_path.stat().st_size if exists_after_check else 0,
            "health": _memory_health(doctor_payload),
        },
        "rbmem_doctor": doctor_payload,
        "registry_size": len(registry),
        "forged_tools": forged_tools,
        "validated_tools": validated_tools,
        "validation_rate": validation_rate,
        "average_success_rate": average_success_rate,
        "category_metrics": category_metrics,
        "debugger_tools": len(debugger_records),
        "debugger_validation_rate": _validation_rate(debugger_records),
        "debugger_average_success_rate": (
            round(sum(debugger_rates) / len(debugger_rates), 4) if debugger_rates else None
        ),
        "declining_sections": declining_sections,
    }


def format_text_report(report: dict[str, Any]) -> str:
    memory_file = report.get("memory_file", {})
    size = memory_file.get("size_bytes", 0)
    lines = [
        "RBForge doctor",
        f"rbforge-version: {report['rbforge_version']}",
        f"rbmem-version: {report['rbmem_cli_version']}",
        f"rbmem-compatible: {report['rbmem_compatibility']['ok']}",
        f"memory: {report['memory_path']}",
        f"memory-health: {memory_file.get('health', 'unknown')} ({size} bytes)",
        f"registry-size: {report['registry_size']}",
        f"forged-tools: {report['forged_tools']}",
        f"validated-tools: {report['validated_tools']}",
        f"validation-rate: {_format_percent(report.get('validation_rate'))}",
        f"average-success-rate: {_format_percent(report.get('average_success_rate'))}",
        f"debugger-tools: {report.get('debugger_tools', 0)}",
        f"debugger-validation-rate: {_format_percent(report.get('debugger_validation_rate'))}",
        (
            "debugger-average-success-rate: "
            f"{_format_percent(report.get('debugger_average_success_rate'))}"
        ),
    ]
    if not memory_file.get("exists_before_check", True) and memory_file.get("exists"):
        lines.append("note: memory file was created during the check")
    declining = report.get("declining_sections", [])
    if declining:
        lines.append(f"\n⚠ Declining sections ({len(declining)}):")
        for s in declining:
            last = s.get("last_used", "N/A")
            lines.append(f"  - {s.get('path', 'unknown')} (last: {last})")
    return "\n".join(lines)


def run_doctor(args: argparse.Namespace) -> dict[str, Any]:
    store = RbmemStore(args.memory_path, rbmem_cli=args.rbmem_cli)
    return build_report(store)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="rbforge doctor")
    subparsers = parser.add_subparsers(dest="command", help="Diagnostic subcommands")

    # Default: run full doctor report
    add_doctor_arguments(parser)

    # health subcommand
    health_parser = subparsers.add_parser("health", help="Show memory health score")
    add_doctor_arguments(health_parser)

    # compact subcommand
    compact_parser = subparsers.add_parser(
        "compact",
        help="Identify and distill stale sections",
    )
    compact_parser.add_argument(
        "--dry-run",
        action="store_true",
        default=False,
        help="Show what would be compacted without making changes.",
    )

    # version subcommand
    version_parser = subparsers.add_parser(
        "version",
        help="Manage RBForge snapshot versions",
    )
    version_parser.add_argument(
        "command",
        nargs="?",
        choices=["list", "create", "diff", "restore"],
        default="list",
        help="Versioning action to perform.",
    )
    version_parser.add_argument(
        "--section",
        default=None,
        help="Section path for snapshot operations.",
    )
    version_parser.add_argument(
        "--id1", "--snapshot1",
        default=None,
        help="First snapshot ID (for diff).",
    )
    version_parser.add_argument(
        "--id2", "--snapshot2",
        default=None,
        help="Second snapshot ID (for diff).",
    )
    version_parser.add_argument(
        "--format",
        choices=("text", "json"),
        default="text",
        help="Output format.",
    )

    # temporal subcommand
    temporal_parser = subparsers.add_parser(
        "temporal",
        help="Time-windowed analysis of section trends",
    )
    temporal_parser.add_argument(
        "--section",
        required=True,
        help="Section path to analyze.",
    )
    temporal_parser.add_argument(
        "--window-hours",
        type=float,
        default=168.0,
        help="Analysis window in hours (default: 168 = 1 week).",
    )
    temporal_parser.add_argument(
        "--format",
        choices=("text", "json"),
        default="text",
        help="Output format.",
    )

    # reviews subcommand
    reviews_parser = subparsers.add_parser(
        "reviews",
        help="Show pending human-in-the-loop reviews",
    )
    reviews_parser.add_argument(
        "--limit",
        type=int,
        default=10,
        help="Max reviews to return.",
    )
    reviews_parser.add_argument(
        "--format",
        choices=("text", "json"),
        default="text",
        help="Output format.",
    )

    # Parse with known args to handle legacy positional memory_path
    # (e.g., main(["custom.rbmem", "--format", "json"]) for backward compat)
    try:
        args, remaining = parser.parse_known_args(argv)

        # If there are remaining positional args and no subcommand,
        # treat the first one as memory_path (legacy style)
        if remaining and not args.command:
            args.memory_path = remaining[0]
    except SystemExit:
        # Legacy mode: first positional arg is memory_path, rest are flags
        # e.g. main(["custom.rbmem", "--format", "json"])
        args = argparse.Namespace()
        args.format = "text"
        args.rbmem_cli = None
        args.command = None
        args.memory_path = "memory.rbmem"
        remaining_iter = iter(argv if argv else [])
        for token in remaining_iter:
            if token == "--format":
                args.format = next(remaining_iter, "text")
            elif token == "--rbmem-cli":
                args.rbmem_cli = next(remaining_iter, None)
            elif not token.startswith("-"):
                args.memory_path = token

    # Dispatch based on subcommand
    if args.command == "health":
        return _cmd_health(args)
    elif args.command == "compact":
        return _cmd_compact(args)
    elif args.command == "version":
        return _cmd_version(args)
    elif args.command == "temporal":
        return _cmd_temporal(args)
    elif args.command == "reviews":
        return _cmd_reviews(args)
    else:
        # Default: full doctor report
        store = RbmemStore(args.memory_path, rbmem_cli=args.rbmem_cli)
        report = build_report(store)
        if args.format == "json":
            print(json.dumps(report, indent=2, sort_keys=True))
        else:
            print(format_text_report(report))
        return 0


def _cmd_health(args: argparse.Namespace) -> int:
    """Health subcommand handler."""
    store = RbmemStore(args.memory_path, rbmem_cli=args.rbmem_cli)
    try:
        payload = store.context("tools custom", resolve=True, minified=False, graph_depth=1)
    except Exception:  # noqa: BLE001 - diagnostics should still use registry fallback
        payload = {"sections": []}

    sections = []
    for section in payload.get("sections", []):
        content = _json_content(section.get("content"))
        if isinstance(content, dict):
            rec = dict(content)
            rec.setdefault("_section_path", section.get("path", ""))
            sections.append(rec)

    report = compute_health_score(sections)
    if args.format == "json":
        import dataclasses
        print(json.dumps(dataclasses.asdict(report), indent=2, sort_keys=True))
    else:
        print(f"Memory Health Score: {report.composite_score:.2f}/1.00")
        for cs in report.component_scores:
            print(f"  {cs.name:25s}: {cs.value:.2f}")
        if report.flags:
            print("\nFlags:")
            for flag in report.flags:
                print(f"  ⚠ {flag}")
        if report.recommendations:
            print("\nRecommendations:")
            for rec in report.recommendations:
                print(f"  → {rec}")
    return 0


def _cmd_compact(args: argparse.Namespace) -> int:
    """Compact subcommand handler."""
    store = RbmemStore(args.memory_path)

    stale = identify_stale_sections(store.read_registry())
    if args.format == "json":
        print(json.dumps({"stale_sections": stale}, indent=2, sort_keys=True))
    else:
        if not stale:
            print("No stale sections found.")
        else:
            print(f"Found {len(stale)} stale sections:")
            for s in stale:
                print(f"  {s.get('path', 'unknown')} "
                      f"(usage={s.get('usage_count', '?')}, "
                      f"last_used={s.get('last_used_at', '?')})")
        if args.dry_run:
            print("\n(dry-run mode — no changes made)")
            return 0
        result = compact_memory(store, dry_run=False)
        print(f"\nCompaction result: {result.sections_scanned} scanned, "
              f"{result.stale_sections_found} stale, "
              f"{len(result.distillation_summaries)} distilled.")
    return 0


def _cmd_version(args: argparse.Namespace) -> int:
    """Version subcommand handler."""
    import dataclasses

    base_dir = "."
    if args.command == "list":
        snap_list = list_snapshots(base_dir=base_dir)
        if args.format == "json":
            print(json.dumps([dataclasses.asdict(s) for s in snap_list.snapshots], indent=2))
        else:
            if not snap_list.snapshots:
                print("No snapshots found.")
            else:
                print(f"Snapshots ({len(snap_list.snapshots)}):")
                for s in snap_list.snapshots:
                    print(f"  {s.snapshot_id:30s} {s.section_path} "
                          f"({s.created_at})")
    elif args.command == "create":
        if not args.section:
            print("Error: --section is required for 'create'", file=__import__("sys").stderr)
            return 1
        snap = create_snapshot(args.section, {}, base_dir=base_dir)
        print(f"Created snapshot: {snap.snapshot_id}")
    elif args.command == "diff":
        if not args.id1 or not args.id2:
            print("Error: --id1 and --id2 required for 'diff'", file=__import__("sys").stderr)
            return 1
        diff = diff_snapshots(args.id1, args.id2, base_dir=base_dir)
        if diff is None:
            print("Error: could not load snapshots", file=__import__("sys").stderr)
            return 1
        if args.format == "json":
            print(json.dumps(dataclasses.asdict(diff), indent=2))
        else:
            print(f"Diff: {diff.snapshot_id_1} → {diff.snapshot_id_2}")
            print(f"  Section: {diff.section_path}")
            print(f"  Fields changed: {diff.fields_changed}")
            print(f"  Added: {diff.added_keys}")
            print(f"  Removed: {diff.removed_keys}")
    elif args.command == "restore":
        print("Restore: use restore_snapshot(id, base_dir=...) directly.")
    return 0


def _cmd_temporal(args: argparse.Namespace) -> int:
    """Temporal subcommand handler."""
    import dataclasses

    store = RbmemStore(args.memory_path)
    sections = store.read_registry()
    results = trend_analysis(sections, window_hours=int(args.window_hours))
    if args.format == "json":
        print(json.dumps([dataclasses.asdict(r) for r in results], indent=2, sort_keys=True))
    else:
        for r in results:
            print(f"Section: {r.section_path}")
            print(f"  Trend: {r.trend}")
            print(f"  Data points: {len(r.data_points)}")
            print(f"  Patterns: {r.patterns}")
    return 0


def _cmd_reviews(args: argparse.Namespace) -> int:
    """Reviews subcommand handler."""
    reviews = get_pending_reviews(
        limit=args.limit,
        memory_path=args.memory_path,
    )
    if args.format == "json":
        import dataclasses
        candidates = [dataclasses.asdict(c) for c in reviews.candidates]
        print(json.dumps({
            "candidates": candidates,
            "total_pending": reviews.total_pending,
            "total_approved": reviews.total_approved,
            "total_rejected": reviews.total_rejected,
        }, indent=2, sort_keys=True))
    else:
        if not reviews.candidates:
            print("No pending reviews.")
        else:
            print(f"Pending reviews ({reviews.total_pending} total):")
            for c in reviews.candidates:
                print(f"  [{c.status}] {c.tool_name} "
                      f"(priority: {c.priority})")
    return 0


def _read_tool_records(
    store: RbmemStore,
    registry: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    seen_sections: set[str] = set()
    try:
        payload = store.context("tools custom", resolve=True, minified=False, graph_depth=1)
    except Exception:  # noqa: BLE001 - diagnostics should still use registry fallback
        payload = {"sections": []}

    for section in payload.get("sections", []):
        path = section.get("path")
        if not isinstance(path, str) or not path.startswith("tools.custom."):
            continue
        content = _json_content(section.get("content"))
        if not isinstance(content, dict):
            continue
        record = dict(content)
        record.setdefault("_section_path", path)
        records.append(record)
        seen_sections.add(path)

    for item in registry:
        if not isinstance(item, dict):
            continue
        section = item.get("section")
        if not isinstance(section, str):
            name = item.get("name")
            section = f"tools.custom.{name}" if isinstance(name, str) else ""
        if section in seen_sections:
            continue
        record = dict(item)
        record["_section_path"] = section
        record["_registry_entry"] = True
        records.append(record)

    return records


def _json_content(raw: Any) -> Any:
    if isinstance(raw, (dict, list)):
        return raw
    if not isinstance(raw, str):
        return None
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return None


def _is_validated(record: dict[str, Any]) -> bool:
    status = str(record.get("status", "")).lower()
    if status in {"validated", "registered"}:
        return True
    return bool(record.get("_registry_entry")) and status in {"", "validated", "registered"}


def _is_debugger_tool(record: dict[str, Any]) -> bool:
    category = str(record.get("category", "")).lower()
    dependencies = record.get("dependencies", [])
    if isinstance(dependencies, str):
        dependency_text = dependencies.lower()
    elif isinstance(dependencies, list):
        dependency_text = " ".join(str(item).lower() for item in dependencies)
    else:
        dependency_text = ""
    return category == "debugger" or "debugger" in dependency_text


def _success_rates(records: list[dict[str, Any]]) -> Sequence[float]:
    rates: list[float] = []
    for record in records:
        metrics = record.get("metrics")
        raw_rate = metrics.get("success_rate") if isinstance(metrics, dict) else record.get(
            "success_rate"
        )
        if isinstance(raw_rate, int | float):
            rates.append(float(raw_rate))
    return rates


def _category_metrics(records: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    categories: dict[str, list[dict[str, Any]]] = {}
    for record in records:
        category = str(record.get("category") or "unknown")
        categories.setdefault(category, []).append(record)
    return {
        category: {
            "tools": len(items),
            "validated_tools": sum(1 for item in items if _is_validated(item)),
            "validation_rate": _validation_rate(items),
            "average_success_rate": _average_success_rate(items),
        }
        for category, items in sorted(categories.items())
    }


def _validation_rate(records: list[dict[str, Any]]) -> float | None:
    if not records:
        return None
    return round(sum(1 for record in records if _is_validated(record)) / len(records), 4)


def _average_success_rate(records: list[dict[str, Any]]) -> float | None:
    rates = list(_success_rates(records))
    return round(sum(rates) / len(rates), 4) if rates else None


def _memory_health(doctor_payload: dict[str, Any]) -> str:
    hermes_load = doctor_payload.get("hermes_load")
    if isinstance(hermes_load, dict) and isinstance(hermes_load.get("status"), str):
        return hermes_load["status"]
    status = doctor_payload.get("status")
    if isinstance(status, str):
        return status
    validation = doctor_payload.get("validation")
    if isinstance(validation, dict) and isinstance(validation.get("status"), str):
        return validation["status"]
    return "unknown"


def _format_percent(value: Any) -> str:
    if not isinstance(value, int | float):
        return "n/a"
    return f"{value * 100:.1f}%"


def _detect_declining_sections(store: DoctorStore) -> list[dict[str, Any]]:
    """Detect sections with declining usage patterns."""
    try:
        payload = store.context("tools custom", resolve=True, minified=False, graph_depth=1)
    except Exception:  # noqa: BLE001 - diagnostics should still use registry fallback
        # rbmem binary doesn't have 'query' — parse file directly
        import re
        import json
        from pathlib import Path
        
        memory_file = Path(store.memory_path)
        content = memory_file.read_text(encoding="utf-8")
        sections = []
        pattern = re.compile(r'\[SECTION: (.+?)\]\n(.*?)\[END SECTION\]', re.DOTALL)
        for match in pattern.finditer(content):
            name = match.group(1).strip()
            body = match.group(2).strip()
            
            temporal = {}
            created_match = re.search(r'created_at:\s*"([^"]+)"', body)
            updated_match = re.search(r'updated_at:\s*"([^"]+)"', body)
            if created_match:
                temporal['created_at'] = created_match.group(1)
            if updated_match:
                temporal['updated_at'] = updated_match.group(1)
            
            if not temporal:
                continue
            
            sections.append({
                'path': name,
                'content': temporal,
            })
        
        declining = []
        for section in sections:
            data = dict(section['content'])
            history = []
            for key in ('created_at', 'updated_at'):
                if data.get(key):
                    history.append({'used_at': data[key]})
            
            data['run_history'] = history
            data['metrics'] = {
                'usage_count': len(history),
                'success_rate': 1.0,
                'last_used_at': data.get('updated_at'),
            }
            
            patterns = usage_pattern(data)
            if 'declining' in patterns:
                declining.append({
                    'path': section['path'],
                    'patterns': patterns,
                    'last_used': data.get('updated_at'),
                    'usage_count': len(history),
                })
        
        return declining

    declining: list[dict[str, Any]] = []
    for section in payload.get("sections", []):
        content = _json_content(section.get("content"))
        if not isinstance(content, dict):
            continue

        # Add temporal metadata as run_history for usage_pattern testing
        temporal = content.get("temporal", {})
        created = temporal.get("created_at")
        updated = temporal.get("updated_at")

        if not created and not updated:
            continue

        history = []
        if created:
            history.append({"used_at": created})
        if updated:
            history.append({"used_at": updated})

        content["run_history"] = history
        content["metrics"] = {
            "usage_count": len(history),
            "success_rate": 1.0,
            "last_used_at": updated,
        }

        patterns = usage_pattern(content)
        if "declining" in patterns:
            declining.append({
                "path": section.get("path", ""),
                "patterns": patterns,
                "last_used": updated,
                "usage_count": len(history),
            })

    return declining


if __name__ == "__main__":
    raise SystemExit(main())

"""Deterministic RBForge evaluation commands."""

from __future__ import annotations

import argparse
import json
from collections.abc import Sequence
from pathlib import Path
from typing import Any

from rbforge_core.debugger import debugger_signal_report

DEFAULT_DEBUGGER_CASES = (
    Path(__file__).resolve().parents[2] / "examples" / "debugger_eval_cases.json"
)


def add_eval_arguments(parser: argparse.ArgumentParser) -> None:
    subcommands = parser.add_subparsers(dest="eval_target", required=True)
    debugger_parser = subcommands.add_parser(
        "debugger",
        help="Compare debugger-first trajectories against no-debugger baselines.",
    )
    add_debugger_eval_arguments(debugger_parser)


def add_debugger_eval_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--cases",
        default=str(DEFAULT_DEBUGGER_CASES),
        help="Path to debugger eval cases JSON.",
    )
    parser.add_argument(
        "--format",
        choices=("text", "json"),
        default="text",
        help="Output format.",
    )


def run_debugger_eval(args: argparse.Namespace) -> dict[str, Any]:
    return build_debugger_eval(load_debugger_cases(Path(args.cases)))


def load_debugger_cases(path: Path) -> list[dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    cases = payload.get("cases") if isinstance(payload, dict) else payload
    if not isinstance(cases, list):
        raise ValueError("debugger eval cases must be a list or an object with a cases list")
    return [case for case in cases if isinstance(case, dict)]


def build_debugger_eval(cases: list[dict[str, Any]]) -> dict[str, Any]:
    results = [_evaluate_case(case) for case in cases]
    case_count = len(results)
    debugger_hits = sum(1 for result in results if result["debugger_root_cause_hit"])
    baseline_hits = sum(1 for result in results if result["baseline_root_cause_hit"])
    debugger_used = sum(1 for result in results if result["debugger_used"])
    reusable_debuggers_created = sum(
        1 for result in results if result["reusable_debugger_created"]
    )
    baseline_turns = sum(result["baseline_turns"] for result in results)
    debugger_turns = sum(result["debugger_turns"] for result in results)
    turn_delta = baseline_turns - debugger_turns
    debugger_scores = [result["debugger_signal_score"] for result in results]
    families = _family_metrics(results)

    return {
        "schema": "rbforge.eval.debugger.v1",
        "cases": case_count,
        "families": len(families),
        "debugger_use_rate": _ratio(debugger_used, case_count),
        "root_cause_hit_rate": _ratio(debugger_hits, case_count),
        "baseline_root_cause_hit_rate": _ratio(baseline_hits, case_count),
        "avg_turn_reduction": _ratio(turn_delta, baseline_turns),
        "baseline_turns": baseline_turns,
        "debugger_turns": debugger_turns,
        "estimated_turns_saved": turn_delta,
        "reusable_debuggers_created": reusable_debuggers_created,
        "avg_debugger_signal_score": (
            round(sum(debugger_scores) / len(debugger_scores), 4) if debugger_scores else None
        ),
        "family_metrics": families,
        "results": results,
    }


def format_debugger_eval(report: dict[str, Any]) -> str:
    return "\n".join(
        [
            "RBForge debugger eval",
            f"cases: {report['cases']}",
            f"families: {report.get('families', 0)}",
            f"debugger-use-rate: {_format_percent(report.get('debugger_use_rate'))}",
            f"root-cause-hit-rate: {_format_percent(report.get('root_cause_hit_rate'))}",
            (
                "baseline-root-cause-hit-rate: "
                f"{_format_percent(report.get('baseline_root_cause_hit_rate'))}"
            ),
            f"avg-turn-reduction: {_format_percent(report.get('avg_turn_reduction'))}",
            f"estimated-turns-saved: {report['estimated_turns_saved']}",
            f"reusable-debuggers-created: {report['reusable_debuggers_created']}",
            (
                "avg-debugger-signal-score: "
                f"{_format_decimal(report.get('avg_debugger_signal_score'))}"
            ),
        ]
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="rbforge eval")
    add_eval_arguments(parser)
    args = parser.parse_args(argv)
    if args.eval_target == "debugger":
        report = run_debugger_eval(args)
        if args.format == "json":
            print(json.dumps(report, indent=2, sort_keys=True))
        else:
            print(format_debugger_eval(report))
        return 0
    parser.error(f"unknown eval target: {args.eval_target}")
    return 2


def _evaluate_case(case: dict[str, Any]) -> dict[str, Any]:
    report = debugger_signal_report(str(case.get("log", "")))
    expected = case.get("expected", {})
    expected = expected if isinstance(expected, dict) else {}
    hit_checks = [
        _matches_expected(report.get("top_exception"), expected.get("exception")),
        _contains_key(report.get("suspect_files"), expected.get("file")),
        _contains_item(report.get("failing_tests"), expected.get("test")),
    ]
    usable_checks = [check for check in hit_checks if check is not None]
    debugger_hit = bool(usable_checks) and all(usable_checks)

    baseline = case.get("baseline", {})
    baseline = baseline if isinstance(baseline, dict) else {}
    debugger = case.get("debugger", {})
    debugger = debugger if isinstance(debugger, dict) else {}

    return {
        "id": str(case.get("id", "")),
        "family": str(case.get("family", "debugging")),
        "debugger_used": bool(debugger.get("used", True)),
        "debugger_root_cause_hit": debugger_hit,
        "baseline_root_cause_hit": bool(baseline.get("root_cause_hit", False)),
        "baseline_turns": int(baseline.get("turns", 0)),
        "debugger_turns": int(debugger.get("turns", 0)),
        "turns_saved": int(baseline.get("turns", 0)) - int(debugger.get("turns", 0)),
        "reusable_debugger_created": bool(debugger.get("reusable_debugger_created", False)),
        "debugger_signal_score": report["debugger_signal_score"],
        "top_exception": report["top_exception"],
        "suspect_files": report["suspect_files"],
        "failing_tests": report["failing_tests"],
    }


def _matches_expected(actual: Any, expected: Any) -> bool | None:
    if not expected:
        return None
    return str(actual) == str(expected)


def _contains_key(mapping: Any, expected: Any) -> bool | None:
    if not expected:
        return None
    return isinstance(mapping, dict) and str(expected) in mapping


def _contains_item(items: Any, expected: Any) -> bool | None:
    if not expected:
        return None
    return isinstance(items, list) and str(expected) in {str(item) for item in items}


def _ratio(numerator: int, denominator: int) -> float | None:
    return round(numerator / denominator, 4) if denominator else None


def _family_metrics(results: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    grouped: dict[str, list[dict[str, Any]]] = {}
    for result in results:
        grouped.setdefault(str(result["family"]), []).append(result)

    metrics: dict[str, dict[str, Any]] = {}
    for family, items in sorted(grouped.items()):
        baseline_turns = sum(int(item["baseline_turns"]) for item in items)
        debugger_turns = sum(int(item["debugger_turns"]) for item in items)
        metrics[family] = {
            "cases": len(items),
            "root_cause_hit_rate": _ratio(
                sum(1 for item in items if item["debugger_root_cause_hit"]),
                len(items),
            ),
            "baseline_root_cause_hit_rate": _ratio(
                sum(1 for item in items if item["baseline_root_cause_hit"]),
                len(items),
            ),
            "avg_turn_reduction": _ratio(baseline_turns - debugger_turns, baseline_turns),
            "turns_saved": baseline_turns - debugger_turns,
        }
    return metrics


def _format_percent(value: Any) -> str:
    if not isinstance(value, int | float):
        return "n/a"
    return f"{value * 100:.1f}%"


def _format_decimal(value: Any) -> str:
    if not isinstance(value, int | float):
        return "n/a"
    return f"{value:.4f}"


if __name__ == "__main__":
    raise SystemExit(main())

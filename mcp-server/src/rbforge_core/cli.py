"""RBForge command line interface."""

from __future__ import annotations

import argparse
from collections.abc import Sequence

from rbforge_core.doctor import add_doctor_arguments, format_text_report, run_doctor
from rbforge_core.eval import add_eval_arguments, format_debugger_eval, run_debugger_eval
from rbforge_core.improver import improve_tool


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="rbforge")
    subcommands = parser.add_subparsers(dest="command", required=True)

    doctor_parser = subcommands.add_parser("doctor", help="Check RBForge and RBMEM health.")
    add_doctor_arguments(doctor_parser)
    eval_parser = subcommands.add_parser("eval", help="Run deterministic RBForge evals.")
    add_eval_arguments(eval_parser)
    improve_parser = subcommands.add_parser("improve", help="Propose or apply tool improvements.")
    improve_parser.add_argument("tool")
    improve_parser.add_argument("memory_path")
    improve_parser.add_argument("--rbmem-cli")
    improve_parser.add_argument("--propose-only", action="store_true")
    improve_parser.add_argument("--auto-apply", action="store_true")

    args = parser.parse_args(argv)
    if args.command == "doctor":
        report = run_doctor(args)
        if args.format == "json":
            import json

            print(json.dumps(report, indent=2, sort_keys=True))
        else:
            print(format_text_report(report))
        return 0
    if args.command == "eval" and args.eval_target == "debugger":
        report = run_debugger_eval(args)
        if args.format == "json":
            import json

            print(json.dumps(report, indent=2, sort_keys=True))
        else:
            print(format_debugger_eval(report))
        return 0
    if args.command == "improve":
        proposal = improve_tool(
            args.tool,
            args.memory_path,
            auto_apply=args.auto_apply and not args.propose_only,
            rbmem_cli=args.rbmem_cli,
        )
        import json

        print(json.dumps(proposal.__dict__, indent=2, sort_keys=True))
        return 0

    parser.error(f"unknown command: {args.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())

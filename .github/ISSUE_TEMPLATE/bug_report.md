---
name: Bug report
about: Report a bug or unexpected behavior
title: '[BUG] '
labels: bug
assignees: ''
---

## Describe the Bug

A clear and concise description of what the bug is.

## To Reproduce

Steps to reproduce the behavior:

1. Run command: `rbmem ...`
2. With input file: [attach or paste minimal `.rbmem` file]
3. See error

## Expected Behavior

A clear and concise description of what you expected to happen.

## Actual Behavior

What actually happened, including any error messages.

## Environment

- **RBMEM version**: `rbmem --version`
- **Rust version**: `rustc --version`
- **OS**: [e.g., Ubuntu 22.04, macOS 14.0, Windows 11]
- **Terminal**: [e.g., bash, zsh, PowerShell]

## Minimal Reproduction

Minimal `.rbmem` file that demonstrates the issue:

```rbmem
# Paste minimal file here
```

## Additional Context

Add any other context about the problem here.

## Logs

If applicable, add logs with `RUST_LOG=debug`:

```bash
RUST_LOG=debug rbmem <command> 2>&1 | head -50
```

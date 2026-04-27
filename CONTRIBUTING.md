# Contributing

This project is currently intended for private GitHub use, but the contribution rules are the same ones expected of a public Rust CLI.

## Local Setup

```powershell
cargo build
cargo test
```

## Quality Bar

- Keep parser code explicit, small, and well commented.
- Preserve backward compatibility for existing `.aif` files.
- Do not allow LLM-provided timestamps to overwrite protected timestamps.
- Prefer focused changes with tests that cover the behavior being changed.
- Keep CLI output stable unless the change is intentional and documented.

Before opening a pull request:

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --all-features
```

If code files changed, refresh the local Graphify output:

```powershell
graphify-rs build --path . --output graphify-out --no-llm --code-only --format report,wiki,json
```

## Pull Request Notes

Include a short description of:

- The behavior changed.
- The compatibility impact.
- Tests or manual commands run.
- Any follow-up work that should not block the PR.

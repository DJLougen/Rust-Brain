# Contributing to RBMEM

Thank you for your interest in contributing to RBMEM! This document provides guidelines and information for contributors.

## Development Setup

```bash
git clone https://github.com/DJLougen/Rust-Brain.git
cd Rust-Brain
cargo build
cargo test
```

## Code Quality Standards

Before submitting a pull request, ensure your changes meet these standards:

### Code Style

- Run `cargo fmt --check` to verify formatting
- Run `cargo clippy --all-targets --all-features -- -D warnings` to catch common issues
- Keep functions focused and small
- Add doc comments for public APIs

### Testing

- Add tests for new functionality
- Ensure all existing tests pass: `cargo test --all-features`
- For parser changes, add regression tests in `tests/parser_regression.rs`
- For CLI changes, add integration tests in `tests/rbmem_cli.rs`

### Backward Compatibility

- Preserve compatibility with existing `.rbmem` files
- Do not allow LLM-provided timestamps to overwrite protected timestamps
- Keep CLI output stable unless the change is intentional and documented
- Update `CHANGELOG.md` for user-facing changes

### Documentation

- Update `README.md` for new features or changed behavior
- Update `docs/ARCHITECTURE.md` for structural changes
- Add examples to `examples/` directory when appropriate
- Update `HERMES.md` for Hermes integration changes

## Pull Request Process

1. **Create a feature branch** from `main`
2. **Make your changes** following the guidelines above
3. **Test thoroughly** with `cargo test --all-features`
4. **Update documentation** as needed
5. **Submit a pull request** with a clear description

### Pull Request Description

Include the following in your PR description:

- **What changed**: Brief description of the changes
- **Why**: Motivation for the change
- **Testing**: Tests added or modified
- **Compatibility impact**: Any breaking changes or migration notes
- **Screenshots**: For CLI output changes, include before/after examples

## Project Structure

```
Rust-Brain/
├── src/
│   ├── main.rs          # CLI entry point
│   ├── lib.rs           # Library exports
│   ├── document.rs      # Core document model
│   ├── parser.rs        # RBMEM parser
│   ├── commands.rs      # Command implementations
│   ├── hermes.rs        # Hermes integration
│   ├── planner/         # SAT planning engine
│   ├── sync.rs          # Markdown sync
│   ├── pack.rs          # Context packs
│   ├── markdown.rs      # Markdown conversion
│   └── server/          # HTTP server
├── tests/               # Integration tests
├── benches/             # Benchmarks
├── examples/            # Example files
└── docs/                # Documentation
```

## Key Design Principles

1. **Timestamp Protection**: Tool-owned timestamps prevent models from inventing history
2. **Section-Level Operations**: Operations work on individual sections, not whole documents
3. **Graph-Aware**: Relationships between sections are first-class citizens
4. **Compact Output**: Minified views for LLM context without losing metadata
5. **Provenance Tracking**: Know where each section came from

## Reporting Issues

When reporting issues, please include:

- RBMEM version (`rbmem --version`)
- Rust version (`rustc --version`)
- Operating system
- Minimal reproduction steps
- Expected vs actual behavior
- Sample `.rbmem` file if relevant

## Questions?

Feel free to open an issue for questions or reach out to the maintainers.

Thank you for contributing to RBMEM!

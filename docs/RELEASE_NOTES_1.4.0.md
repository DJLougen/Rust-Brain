# Rust-Brain / RBMEM 1.4.0 Release Notes

## Highlights

- Rust-Brain now exposes a public `rbmem` library API in addition to the CLI.
- New RBMEM writes use format version `1.4.0`.
- Existing RBMEM v1.3 files still parse and normalize with `_source_version`.
- Section-level AES-256-GCM encryption is available through `encrypt`, `decrypt`, and `--decrypt` read/query flows.
- Section-level diff, three-way merge, graph export, indexing, and Axum server mode are available.
- CLI observability now supports `--log-format json` with `RUST_LOG`.

## Compatibility

RBMEM v1.3 files remain loadable. When older documents are parsed or migrated,
Rust-Brain preserves the original version in `_source_version` and writes the
current document as RBMEM `1.4.0`.

## Validation

Validated with:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- Real CLI smoke flow for create, update, read, encrypt, decrypt, diff, merge,
  export, Markdown conversion, doctor JSON, and JSON logging.

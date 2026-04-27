## Summary

Describe the change and why it is needed.

## Compatibility

- [ ] Existing `.aif` files continue to parse.
- [ ] CLI output changes are intentional and documented.
- [ ] Protected timestamp behavior is preserved.

## Tests

Commands run:

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --all-features
```

## Notes

Add any follow-up work or review focus areas.

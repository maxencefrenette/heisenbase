# AGENTS Instructions

To contribute to this repository:

- Run `cargo fmt -- --check` to ensure code is formatted.
- Run `cargo test` and ensure all tests pass.
- The integration test in `tests/wdl_file_roundtrip.rs` is ignored by default. Run it explicitly with `cargo test -- --ignored`.
- Keep pull request messages concise and mention the tests executed.

These instructions apply to all files in this repository.

## Compression

Compression is not yet implemented, but it will use an algorithm similar to the one in the Syzygy tablebase generator (`tb`) repository.
The main differences are that we store a different set of values (see `wdl_score_range.rs`) and we only store a WDL table.

See [docs/syzygy_compression.md](docs/syzygy_compression.md) for more details.

## Generating tablebase files

As a test, the KQvK table can be generated with the following command:

```bash
cargo run --release -- generate KQvK
```

This command generates a file called `KQvK.hbt` in the `data` directory.

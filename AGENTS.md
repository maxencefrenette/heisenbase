# AGENTS Instructions

To contribute to this repository:

- Run `cargo fmt -- --check` to ensure code is formatted.
- Run `cargo test` and ensure all tests pass.
- Keep pull request messages concise and mention the tests executed.

These instructions apply to all files in this repository.

## Compression

Compression is not yet implemented, but will use an algorithm similar to the one used in the Syzygy tablebase generator (tb) repository. The main differences are that we store a different set of values (see wdl_score_range.rs) and we only store a wdl table.

See [docs/syzygy_compression.md](docs/syzygy_compression.md) for more details.

## Generating tablebase files

As a test, the KQvK table can be generated with the following command:

```bash
cargo run --release -- generate KQvK
```

This will generate a file called `KQvK.hbt` in the `data` directory.

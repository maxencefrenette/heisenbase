# AGENTS Instructions

To contribute to this repository:

- Run `cargo fmt` to ensure code is formatted.
- Run `cargo test` and ensure all tests pass.
- The integration test in `tests/wdl_file_roundtrip.rs` is ignored by default. Run it explicitly with `cargo test -- --ignored`.
  - Those tests take longer to run, only run them when making big changes
- Keep pull request messages concise and mention the tests executed.

These instructions apply to all files in this repository.

## Generating tablebase files

As a test, the KQvK table can be generated with the following command:

```bash
cargo run --release -- generate KQvK
```

This command generates a file called `KQvK.hbt` in the `data` directory.

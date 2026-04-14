# AGENTS Instructions

- Run `cargo fmt` to ensure code is formatted.
- Run `cargo clippy --all-targets --all-features -- -D warnings` and ensure it passes.
- Run `cargo test` and ensure all tests pass.

## Generating tablebase files

As a test, the KQvK table can be generated with the following command:

```bash
cargo run --release -- generate KQvK
```

This command stores the generated table in `data/heisenbase.db`.

## Cutovers

Temporary data is stored in the gitignored `data/` directory of this repository. When making changes to the data storage format, there is never any need for a cutover. Simply wipe the data and start fresh. Never write a database migration in the code.

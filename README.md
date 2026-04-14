# heisenbase
Fuzzy Chess EGTB

As part of this project, I'm trying to push the limits of how much I can get AI to write code, so
please forgive the AI slop.

## Git hooks
This repo uses `prek` with a pre-commit config to run:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`

Enable the hooks with:

```bash
prek install
```

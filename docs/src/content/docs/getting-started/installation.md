---
title: Installation
description: Build the current FScript toolchain from source and run the CLI locally.
---

The repository currently builds the FScript CLI and runtime tooling from source with Cargo.

## Build and install

```sh
cargo install --path crates/fscript-cli --bin fscript
```

## Inspect the CLI

```sh
fscript --help
```

## Notes

- The CLI binary will be created in `target/debug/fscript` after building.
- The Cargo package is `fscript-cli`.
- The compiled binary is `fscript`.
- The current CLI exposes `check`, `run`, and `compile`.


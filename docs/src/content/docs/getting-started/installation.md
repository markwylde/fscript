---
title: Installation
description: Build the current FScript toolchain from source and run the CLI locally.
---

The repository currently builds the FScript CLI and runtime tooling from source with Cargo.

## Prerequisites

You need:

- a recent stable Rust toolchain
- Cargo
- a system linker suitable for Rust native builds

FScript is implemented as a Rust workspace, so the fastest way to get started is to build and run the CLI directly from the repo.

## Clone and build

```bash
git clone <repo-url>
cd fscript
cargo build
```

That builds the workspace and produces the `fscript` CLI.

## Run the CLI without installing globally

```bash
cargo run -p fscript-cli -- --help
```

Current top-level commands:

- `fscript check`
- `fscript run`
- `fscript compile`
- `fscript version`

## Useful first commands

Validate a file:

```bash
cargo run -p fscript-cli -- check examples/hello.fs
```

Run a file:

```bash
cargo run -p fscript-cli -- run examples/hello.fs
```

Compile a file:

```bash
cargo run -p fscript-cli -- compile examples/hello.fs ./hello
```

## Current status note

`run` currently has broader feature coverage than the real native part of `compile`. The compiler can still emit executables for a broader subset through the embedded-runner bridge, but the fully native backend is intentionally documented as narrower for now.

## If you are coming from Node.js

There is no `npm install -g fscript` flow documented yet because the toolchain is still Draft 0.1 and actively evolving. Treat the repo itself as the source of truth and build it with Cargo.

## Next step

Continue to [Your First Program](./your-first-program.md) once the CLI is building locally.

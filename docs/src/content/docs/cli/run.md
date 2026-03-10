---
title: fscript run
description: Execute an FScript entrypoint through the current runtime and interpreter path.
---

# `fscript run`

`fscript run` executes an FScript entrypoint through the current shared IR, runtime, and interpreter path.

## Usage

```text
Usage: fscript run <PATH>
```

Example:

```bash
cargo run -p fscript-cli -- run src/main.fs
```

## Why this is the default execution command today

The implementation plan treats `run` as the main source of truth for behavior while the native compiler continues to grow. That means `run` is currently the broadest supported execution path.

Today that path includes substantial support for:

- user-defined functions and currying
- records and arrays
- `if`, `match`, destructuring, and generators
- `try/catch`, `throw`, and `defer`
- runtime-backed `std:` modules
- user-module imports with cycle rejection and once-per-module initialization

## When to prefer it

Use `run` when:

- you want the most complete current behavior
- you are testing program semantics instead of binary output
- you want to validate that runtime-backed `std:` operations behave as expected

## Comparison to JavaScript

This is not "run the transpiled JavaScript." The language runtime is its own system, implemented in Rust, with its own semantics for effects, tasks, deferred work, and module loading.

## Related pages

- [Execution Model](../runtime/execution-model.md)
- [fscript compile](./compile.md)
- [Compile vs Run](../implementation-status/compile-vs-run.md)

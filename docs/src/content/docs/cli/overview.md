---
title: CLI Overview
description: The current FScript command-line surface and how the main commands fit together.
---

# CLI Overview

The current FScript CLI is a small tool with three main commands:

- `check`
- `run`
- `compile`

The help output describes it as:

```text
FScript compiler and tooling
```

## Command Summary

### `fscript check <file.fs>`

Typecheck and validate a source file.

### `fscript run <file.fs>`

Run an FScript entrypoint.

### `fscript compile <input.fs> <output>`

Compile an FScript entrypoint to a native executable.

## Practical Advice

- start with `check` when you want fast validation
- use `run` for the broadest current execution path
- use `compile` when your program is within the currently supported compile subset

## Related Pages

- [Check](./check.md)
- [Run](./run.md)
- [Compile](./compile.md)
- [Compile vs run](../implementation-status/compile-vs-run.md)

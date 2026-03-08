---
title: Compile vs Run
description: Why the current run path is broader than the current compile path.
---

# Compile vs Run

The most important current implementation caveat is simple: `run` supports more than `compile`.

## Why

The implementation plan intentionally builds `fscript run` on top of the shared IR interpreter first, then grows native code generation after that path is stable.

That means:

- `run` is the main execution path for the broader supported subset
- `compile` is real and useful, but narrower
- docs should never imply parity that does not exist yet

## Recommended Workflow

1. use `check` for validation
2. use `run` to confirm behavior
3. use `compile` when your program is in the currently supported backend subset

## Related Pages

- [CLI run](../cli/run.md)
- [CLI compile](../cli/compile.md)
- [Supported features](./supported-features.md)

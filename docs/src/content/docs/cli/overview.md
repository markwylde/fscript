---
title: CLI Overview
description: The current FScript command-line surface and how the main commands fit together.
---

# CLI Overview

The current CLI exposes four top-level commands:

- `fscript check`
- `fscript run`
- `fscript compile`
- `fscript version`

The command help currently looks like this:

```text
Usage: fscript [COMMAND]

Commands:
  check    Typecheck and validate a source file
  run      Run an FScript entrypoint
  compile  Compile an FScript entrypoint to a native executable
  version  Show version and build information
```

## How the commands fit together

`check`
: validate a program without executing it

`run`
: execute through the broadest current runtime path

`compile`
: build an executable and exercise the current native pipeline

`version`
: inspect build and version metadata

## A practical loop

For most projects today, this is the smoothest order:

1. `fscript check src/main.fs`
2. `fscript run src/main.fs`
3. `fscript compile src/main.fs ./main`

That sequence matches the current maturity of the implementation: validation is strong, runtime execution is broader, and compile parity is still expanding.

## Important current limitation

The docs distinguish carefully between the language design and the current implementation. In particular:

- `run` is the most capable execution path today
- `compile` already emits real executables
- the fully native backend still covers a smaller slice than the long-term language design

Read [Compile vs Run](../implementation-status/compile-vs-run.md) if you want the exact framing.

## Comparison to TypeScript tooling

This CLI is intentionally smaller than a typical TypeScript toolchain:

- no separate bundler is required by the language
- no Node.js runtime is assumed by the language model
- the compiler is not primarily a JavaScript transpiler

Think of FScript more like a small native language toolchain than a JS ecosystem wrapper.

## Command reference

- [fscript check](./check.md)
- [fscript run](./run.md)
- [fscript compile](./compile.md)

---
title: fscript check
description: Validate and typecheck an FScript source file.
---

# `fscript check`

`fscript check` validates a source file and its module graph without executing the program.

## Usage

```text
Usage: fscript check <PATH>
```

Example:

```bash
cargo run -p fscript-cli -- check src/main.fs
```

## What it validates

The current implementation checks:

- lexing
- parsing
- name resolution
- typechecking
- effect analysis
- user-module import graph rules

That makes it the best first command to run while editing.

## When to use it

Use `check` when:

- you want compiler feedback without triggering effects
- you are iterating on types or module structure
- you want CI-friendly validation

## What it does not do

`check` does not:

- execute `main`
- run filesystem or other host effects
- prove that every runtime-backed boundary value is semantically correct after parsing external data

For example, JSON parsing and data validation are still boundary concerns that your program must model explicitly.

## Comparison to TypeScript

If you know `tsc --noEmit`, this is the closest equivalent in everyday workflow. The main difference is that FScript also includes effect analysis and module-graph validation as part of that single command.

## Related pages

- [fscript run](./run.md)
- [fscript compile](./compile.md)
- [Supported Features](../implementation-status/supported-features.md)

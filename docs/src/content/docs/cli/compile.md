---
title: fscript compile
description: Compile an entrypoint to a native executable, with important current limitations.
---

# `fscript compile`

`fscript compile` builds an FScript entrypoint into a native executable.

## Usage

```text
Usage: fscript compile <INPUT> <OUTPUT>
```

Example:

```bash
cargo run -p fscript-cli -- compile src/main.fs ./main
```

## What "compile" means today

The current compile pipeline is intentionally described as mixed:

- there is a real Cranelift-backed native codegen slice
- broader compile coverage is still supported through an embedded-runner bridge
- full parity with `run` is still a roadmap item

That is why the docs talk about compile support carefully. The command is real and useful, but not every successful `run` program is already handled by the fully native backend.

## Good use cases right now

- producing executables for supported programs
- testing the native pipeline as it expands
- validating that your project stays within current compile coverage

## When to sanity-check with `run`

If a program is important and uses richer language features, it is wise to test both:

1. `fscript check`
2. `fscript run`
3. `fscript compile`

That gives you both semantic confidence and a compile-coverage signal.

## Comparison to TypeScript builds

Unlike `tsc`, FScript is not mainly compiling to JavaScript output. The intended end state is a native toolchain and runtime. The current mixed pipeline is a temporary implementation stage, not the design goal.

## Related pages

- [Compile vs Run](../implementation-status/compile-vs-run.md)
- [Roadmap](../implementation-status/roadmap.md)

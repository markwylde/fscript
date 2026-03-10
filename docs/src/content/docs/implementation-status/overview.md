---
title: Implementation Status
description: How to read the docs when the language and implementation are still Draft 0.1.
---

# Implementation Status

FScript is documented as a language with a clear Draft 0.1 direction, but the repository is still in an implementation-building phase. That means two things are true at once:

- the docs describe the intended language and runtime model
- some features are more complete in the current toolchain than others

## How to read the rest of the docs

Use the docs in three layers:

1. language and type-system pages explain the intended model
2. CLI and runtime pages explain the current shipped behavior
3. implementation-status pages explain where parity is still expanding

## Current high-level reality

Today the project already has:

- a substantial parser and semantic frontend
- strict typechecking for the implemented executable subset
- effect analysis
- a shared runtime and interpreter path for `run`
- runtime-backed `std:` modules
- a mixed compile pipeline with a real native backend slice plus broader bridge coverage

The remaining gap is not "nothing works yet." The remaining gap is that some parts of the language are further along than others, especially on the fully native compile path.

## Good expectation setting

If you are evaluating the project right now:

- trust `check` for compiler validation work
- treat `run` as the broadest current execution path
- treat `compile` as useful and real, but still growing toward full parity

## Related pages

- [Supported Features](./supported-features.md)
- [Compile vs Run](./compile-vs-run.md)
- [Roadmap](./roadmap.md)

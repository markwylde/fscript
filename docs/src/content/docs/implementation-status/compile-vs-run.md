---
title: Compile vs Run
description: Why the current run path is broader than the current compile path.
---

# Compile vs Run

One of the most important current implementation details is that `fscript run` and `fscript compile` do not yet have identical internals.

## `run`

`run` is built on the shared frontend, shared IR, runtime, and interpreter path. It is the broadest current execution route and the main behavioral reference point while the compiler matures.

## `compile`

`compile` already produces executables, but the pipeline is mixed:

- a real Cranelift-backed native slice exists
- broader coverage is still supported by packaging the program into an embedded-runner bridge

## Why this matters

A program that works with `run` may not yet exercise the fully native backend, even if `compile` can still build an executable for it through the bridge.

That is not a contradiction. It is simply a staging strategy:

- keep one shared execution truth for behavior
- expand real native lowering gradually
- avoid inventing multiple incompatible semantics

## Recommended habit

When testing an important program:

1. `fscript check`
2. `fscript run`
3. `fscript compile`

This gives you the clearest signal about both language correctness and current compiler maturity.

## Long-term goal

The roadmap is for the compile pipeline to grow until the broader bridge becomes unnecessary and the real native path owns the full supported language subset.

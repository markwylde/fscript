---
title: Supported Features
description: A high-level summary of what the current repository already implements and where the edges still are.
---

# Supported Features

Based on the implementation plan and current repository surface, the project already includes a substantial executable slice.

## Broadly Present Today

- CLI wiring for `check`, `run`, and `compile`
- lexing and parsing for a broad language surface
- name resolution, HIR lowering, typechecking, and effect analysis for the current supported subset
- shared IR and interpreter support for the main `run` path
- user-module imports with path resolution and cycle rejection
- runtime-backed `std:json`, `std:logger`, and `std:filesystem`
- comment-tolerant JSON parsing plus compact and pretty JSON output
- core examples running through `fscript run`

## Areas Still Evolving

- scheduler-backed effect execution
- full parity between `run` and `compile`
- broader standard-library completeness relative to representative spec APIs
- some parts of the runtime model that are described more fully in the specs than in the current executable implementation

## A Small Extra Note

The repository also contains early support for `std:http`, used by the HTTP hello server example, but it is beyond the minimum Draft 0.1 core stdlib set.

## Related Pages

- [Compile vs run](./compile-vs-run.md)
- [CLI overview](../cli/overview.md)
- [Runtime scheduler](../runtime/scheduler.md)

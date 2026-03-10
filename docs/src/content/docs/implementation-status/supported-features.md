---
title: Supported Features
description: A high-level summary of what the current repository already implements and where the edges still are.
---

# Supported Features

The repository already implements a meaningful slice of the language and runtime. This page is a high-level summary, not an exhaustive matrix.

## Frontend and validation

Currently supported in the frontend:

- imports and exports
- type declarations
- bindings and destructuring
- functions and currying
- records, arrays, calls, pipes, and control flow
- generator syntax
- name resolution
- strict typechecking for the current executable subset
- effect analysis

## `run` path

The current `run` path supports a broad slice of executable behavior, including:

- user-defined functions
- records and arrays
- `if`, `match`, destructuring, and generators
- `try/catch` and `throw`
- memoized `defer`
- user-module imports
- runtime-backed `std:json`, `std:filesystem`, minimal `std:http`, and `std:task`

## `compile` path

The current compile story includes:

- standalone executable output
- a real bounded Cranelift/object/link path
- a broader embedded-runner bridge for wider behavior coverage

## Important caveats

- not every spec idea has full implementation parity yet
- long-lived background dependency draining in the scheduler is still an active gap
- the real native backend still covers less than the full long-term language surface

## Best mental model

Think of the project as "already usable in meaningful slices, still converging on its final architecture," not as "purely aspirational" and not as "already feature-complete."

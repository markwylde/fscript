---
title: Runtime Generators
description: How the runtime manages generator state for lazy sequences.
---

# Runtime Generators

Generators are runtime-managed lazy sequences.

## Runtime responsibilities

- store generator state
- preserve captured locals
- resume from the last `yield`
- report either the next yielded value or completion

## Recommended representation

The spec's guidance is:

- a generator frame
- an instruction pointer or state index
- captured locals
- a completion flag

## Important limit

Draft 0.1 generators are for pure lazy iteration, not async streaming.

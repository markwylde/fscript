---
title: Runtime Generators
description: How the runtime manages generator state for lazy sequences.
---

# Runtime Generators

Generators are runtime-managed lazy sequences.

## Runtime Responsibilities

- store generator state
- resume execution from the last `yield`
- produce the next yielded value or completion
- preserve captured environments

## Draft 0.1 Boundaries

- generators are for pure lazy iteration
- they do not model async streaming
- async streams are a separate future abstraction

## Related Pages

- [Language generators](../language-guide/generators.md)
- [Function types](../type-system/functions.md)

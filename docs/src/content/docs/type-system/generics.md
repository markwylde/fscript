---
title: Generics
description: Reusable type parameters for arrays, results, sequences, and other common abstractions.
---

Generics let you write reusable abstractions without giving up type precision.

## Examples

```fscript
type Maybe<T> = T | Null
type Result<T, E> =
  | { tag: 'ok', value: T }
  | { tag: 'error', error: E }
```

## Where you will see them most

- arrays like `T[]`
- results like `Result<T, E>`
- sequences like `Sequence<T>`
- standard-library helpers such as `Array.map`

## Comparison to TypeScript

The syntax is intentionally familiar. Draft 0.1 focuses on the straightforward reusable cases that make value-oriented code ergonomic.

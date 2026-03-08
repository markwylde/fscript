---
title: Generators
description: Generator arrows, yield, and lazy pure sequences in Draft 0.1.
---

Generators produce lazy sequences.

```fs
numbers = *(): Sequence<Number> => {
  yield 1
  yield 2
  yield 3
}
```

## Rules

- a generator arrow produces `Sequence<T>`
- `yield expr` must match the element type
- generators are intended for pure lazy iteration in Draft 0.1
- yielding effectful work is a type or effect error


---
title: Generators
description: Generator arrows, yield, and lazy pure sequences in Draft 0.1.
---

Generators produce lazy sequences.

## Generator arrow syntax

```fscript
numbers = *(): Sequence<Number> => {
  yield 1
  yield 2
  yield 3
}
```

## Key rules

- generator arrows return `Sequence<T>`
- `yield` values must match the sequence element type
- generators are intended for pure lazy iteration in Draft 0.1
- yielding effectful work is a type or effect error

## Why the rules are strict

FScript keeps generators separate from async streaming so the semantics stay predictable. They model lazy pure production, not background IO.

## Comparison to JavaScript

The syntax will look familiar, but the role is narrower and more disciplined than JS generators.

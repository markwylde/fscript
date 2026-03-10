---
title: Generators and Sequences
description: Use generator arrows to model pure lazy sequences.
---

# Generators and Sequences

Generators in FScript are for pure lazy iteration. They are not the same thing as async streams.

## Example: a small sequence

```fscript
numbers = *(): Sequence<Number> => {
  yield 1
  yield 2
  yield 3
}
```

## Example: derived sequence

```fscript
countFrom = (start: Number): Sequence<Number> => *() => {
  yield start
  yield start + 1
  yield start + 2
}
```

## Why generators matter

They let you describe ordered lazy values without giving up the rest of the language model:

- closures still work
- the yielded values are typed
- the runtime manages generator state explicitly

## Important rule

Draft 0.1 treats generators as pure. Yielding effectful work is a type or effect error.

That means generators are great for:

- derived sequences
- reusable iteration logic
- pure lazy data production

They are not the tool for:

- filesystem streaming
- network subscriptions
- background async workflows

## Comparison to JavaScript

The syntax will feel familiar if you know JS generators, but the role is narrower. FScript uses generators for pure lazy sequences and keeps effectful streaming as a separate future abstraction.

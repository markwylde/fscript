---
title: Inference
description: What FScript infers automatically and where explicit annotations are expected in Draft 0.1.
---

FScript infers local binding types and many local return types when the result is clear.

## Examples

```fscript
answer = 42
name = 'Ada'
active = true
```

The compiler can infer `Number`, `String`, and `Boolean` here.

## Where Draft 0.1 expects annotations

- function parameters should usually be annotated
- exported functions should usually have explicit return types
- recursive functions may need explicit annotations when inference would be unstable

## Why the split exists

This gives you a useful middle ground:

- local code stays lightweight
- module boundaries stay readable
- early compiler implementation stays simpler and more predictable

## Comparison to TypeScript

TypeScript can infer a lot, but it also allows more escape hatches. FScript is intentionally stricter because the type system is meant to support stronger compile-time guarantees.

---
title: Inference
description: What FScript infers automatically and where explicit annotations are expected in Draft 0.1.
---

FScript infers local binding types and many local return types when the result is clear.

```fs
answer = 42
name = 'Ada'
add = (a: Number, b: Number) => a + b
```

## Draft 0.1 guidance

- local immutable bindings may omit annotations
- function parameters should usually be annotated
- local return types may be inferred
- exported functions should have explicit return types


---
title: Unknown, Never, Null, and Undefined
description: Four special built-in types that mark boundaries, unreachable states, and explicit absence.
---

These types each have a distinct role.

## `Unknown`

Use `Unknown` for values whose shape is not yet trusted. It is especially useful at host boundaries before parsing or validation has finished.

## `Never`

`Never` represents code paths that do not produce a value, such as a helper that always throws.

```fscript
fail = (message: String): Never => {
  throw { tag: 'fatal', message }
}
```

## `Null`

`Null` is an explicit value-level absence marker. It is not silently assignable everywhere.

## `Undefined`

`Undefined` is also explicit and distinct from `Null`. It often appears as the result of helpers that exist for side effects rather than meaningful values.

## Why the distinction matters

FScript does not want JavaScript's blurred absence model. Keeping these types separate improves clarity and type safety.

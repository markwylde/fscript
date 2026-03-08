---
title: std:object
description: Immutable record composition and explicit object helpers.
---

# `std:object`

`std:object` provides explicit record helpers without relying on prototypes.

```fs
import Object from 'std:object'
```

## Representative API

```fs
Object.spread = <T>(parts: T[]): T
Object.keys = <T>(value: T): String[]
Object.values = <T>(value: T): Unknown[]
Object.has = <T>(key: String, value: T): Boolean
Object.set = <T, V>(key: String, fieldValue: V, value: T): T
```

## Example

```fs
import Object from 'std:object'

base = { a: 1 }
next = Object.spread(base, { b: 2 })
```

## Current Implementation Note

The current runtime-backed implementation already exposes `Object.spread`.

## Related Pages

- [Bindings and immutability](../language-guide/bindings-and-immutability.md)
- [Record types](../type-system/records.md)

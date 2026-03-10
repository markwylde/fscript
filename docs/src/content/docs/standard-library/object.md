---
title: std:object
description: Immutable record composition and explicit object helpers.
---

# `std:object`

`std:object` provides explicit helpers for working with records.

```fscript
import Object from 'std:object'
```

## Representative API

```fscript
Object.spread = <T>(parts: T[]): T
Object.keys = <T>(value: T): String[]
Object.values = <T>(value: T): Unknown[]
Object.entries = <T>(value: T): { key: String, value: Unknown }[]
Object.has = <T>(key: String, value: T): Boolean
Object.get = <T>(key: String, value: T): Unknown | Null
Object.set = <T, V>(key: String, fieldValue: V, value: T): T
```

## Example

```fscript
base = { id: '1', name: 'Ada' }
next = Object.spread(base, { active: true })
```

## Notes

- record updates preserve immutability
- `Object.spread` should merge left to right
- dynamic-key helpers may be conservative at the type level in Draft 0.1

## Comparison to JavaScript

This is the explicit alternative to object spread plus prototype-era habits. Records stay plain, and helpers stay imported.

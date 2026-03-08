---
title: std:number
description: Numeric helpers and string-to-number parsing.
---

# `std:number`

`std:number` provides numeric helpers and parsing-related functions.

```fs
import Number from 'std:number'
```

## Representative API

```fs
Number.parse = (value: String): Number
Number.toString = (value: Number): String
Number.floor = (value: Number): Number
Number.ceil = (value: Number): Number
Number.round = (value: Number): Number
Number.clamp = (min: Number, max: Number, value: Number): Number
```

## Example

```fs
import Number from 'std:number'

port = Number.parse('8080')
```

## Current Implementation Note

The current runtime-backed implementation already exposes `Number.parse`.

## Related Pages

- [Primitive types](../type-system/primitive-types.md)
- [Result module](./result.md)

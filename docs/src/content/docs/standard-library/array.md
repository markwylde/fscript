---
title: std:array
description: Immutable collection helpers designed for pipes and partial application.
---

# `std:array`

`std:array` provides immutable array construction and transformation helpers.

```fs
import Array from 'std:array'
```

## Representative API

```fs
Array.map = <T, U>(fn: (value: T): U, items: T[]): U[]
Array.filter = <T>(fn: (value: T): Boolean, items: T[]): T[]
Array.reduce = <T, U>(fn: (state: U, value: T): U, initial: U, items: T[]): U
Array.length = <T>(items: T[]): Number
Array.append = <T>(value: T, items: T[]): T[]
```

## Example

```fs
import Array from 'std:array'

names = users
  |> Array.filter((user) => user.active)
  |> Array.map((user) => user.name)
```

## Current Implementation Note

The current runtime-backed implementation already exposes `map`, `filter`, and `length`. Other APIs documented here are part of the intended Draft 0.1 module shape.

## Related Pages

- [Pipes](../language-guide/pipes.md)
- [Array types](../type-system/arrays.md)

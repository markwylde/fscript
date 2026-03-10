---
title: std:array
description: Immutable collection helpers designed for pipes and partial application.
---

# `std:array`

`std:array` provides immutable collection helpers for ordered data.

```fscript
import Array from 'std:array'
```

## Representative API

```fscript
Array.map = <T, U>(fn: (value: T): U, items: T[]): U[]
Array.filter = <T>(fn: (value: T): Boolean, items: T[]): T[]
Array.reduce = <T, U>(fn: (state: U, value: T): U, initial: U, items: T[]): U
Array.forEach = <T>(fn: (value: T): Undefined, items: T[]): Undefined
Array.length = <T>(items: T[]): Number
Array.append = <T>(value: T, items: T[]): T[]
Array.concat = <T>(right: T[], left: T[]): T[]
Array.at = <T>(index: Number, items: T[]): T | Null
Array.slice = <T>(start: Number, end: Number, items: T[]): T[]
Array.flatMap = <T, U>(fn: (value: T): U[], items: T[]): U[]
```

## Example

```fscript
names = users
  |> Array.filter((user) => user.active)
  |> Array.map((user) => user.name)
```

## Why the API looks this way

- curried by default
- data-last for pipe friendliness
- immutable updates

## Comparison to JavaScript

This module replaces prototype calls such as `.map`, `.filter`, and `.reduce`.

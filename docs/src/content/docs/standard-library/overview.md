---
title: Standard Library Overview
description: Explicit std modules, curried data-last APIs, and runtime-backed host capabilities.
---

# Standard Library Overview

FScript's standard library is explicit by design. Helpers live in imported `std:` modules rather than on prototype chains or global namespaces.

## Core principles

- standard-library modules use the reserved `std:` scheme
- default imports are the normal pattern for `std:` modules
- functions are curried by default
- transformation helpers are usually data-last
- collection helpers never mutate caller-visible data

## Example

```fscript
import Array from 'std:array'

names = users
  |> Array.filter((user) => user.active)
  |> Array.map((user) => user.name)
```

That style works because `Array.filter` and `Array.map` are designed for pipes and partial application.

## Core Draft 0.1 modules

- `std:array`
- `std:object`
- `std:string`
- `std:number`
- `std:result`
- `std:json`
- `std:logger`
- `std:filesystem`
- `std:task`

## Comparison to JavaScript

Instead of `items.map(...)` or `text.trim()`, FScript uses imported helpers such as `Array.map(...)` and `String.trim(...)`. That keeps behavior explicit and avoids depending on prototype methods.

## Current implementation note

The current runtime already ships substantial runtime-backed `std:` support, including filesystem, JSON, logging, and task helpers. As with the rest of Draft 0.1, the documented surface is intentionally small and focused.

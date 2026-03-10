---
title: Function Types
description: Curried function types, partial application, and parameter or result typing in FScript.
---

Functions are first-class and curried by default.

## Example

```fscript
add = (a: Number, b: Number): Number => a + b
```

Semantically this behaves like:

```text
Number -> Number -> Number
```

## Rules

- multi-parameter functions are curried automatically
- partial application is valid when argument types line up
- extra arguments are a type error
- functions are values, but not structurally comparable with `===`

## Why this matters

Function types show up everywhere in the standard library because pipes and partial application are central to the language style.

---
title: Pipes
description: Compose transformations left to right with the pipe operator and data-last helper functions.
---

The pipe operator keeps transformation code readable by letting values flow left to right.

## Example

```fscript
import Array from 'std:array'

names = users
  |> Array.filter((user) => user.active)
  |> Array.map((user) => user.name)
```

## Why FScript leans on pipes

Pipes pair naturally with:

- curried functions
- data-last standard-library helpers
- expression-oriented blocks

That makes multi-step transformations read like a sequence of small decisions rather than nested function calls.

## Comparison to JavaScript

JavaScript often relies on method chains for this style. FScript gets a similar readability benefit while keeping helper functions explicit and separate from value objects.

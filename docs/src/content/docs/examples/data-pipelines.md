---
title: Data Pipelines
description: Compose immutable transformations with arrays, pipes, and curried functions.
---

# Data Pipelines

FScript is a strong fit for data reshaping work because the language defaults line up with transformation pipelines:

- values are immutable
- array helpers are curried
- pipes keep the flow left to right

## Example: active user names

```fscript
import Array from 'std:array'
import String from 'std:string'

type User = {
  id: String,
  name: String,
  active: Boolean,
}

displayNames = (users: User[]): String[] => users
  |> Array.filter((user) => user.active)
  |> Array.map((user) => String.uppercase(user.name))
```

## Why this style works well

Each step does one thing:

- `Array.filter` narrows the collection
- `Array.map` reshapes the values
- the pipe makes the order read like a recipe

Because the APIs are data-last, partial application is natural:

```fscript
onlyActive = Array.filter((user: User) => user.active)
toUpperName = Array.map((user: User) => String.uppercase(user.name))

displayNames = (users: User[]): String[] => users
  |> onlyActive
  |> toUpperName
```

## Comparison to JavaScript

A JavaScript version would likely use `users.filter(...).map(...)`. FScript chooses imported helpers instead so the behavior is explicit and consistent across the language.

## Tips for good pipelines

- keep each pipeline stage small
- extract named helpers when a lambda gets busy
- perform validation near the boundary before entering the pure pipeline
- return new records instead of trying to "patch" old ones in place

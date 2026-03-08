---
title: Syntax Overview
description: A quick tour of the main FScript syntax forms before diving into each feature in detail.
---

FScript uses a small, expression-oriented syntax with arrows, immutable bindings, records, arrays, pattern matching, and explicit imports.

```fs
import Array from 'std:array'

type User = { name: String, active: Boolean }

names = (users: User[]): String[] => {
  users
    |> Array.filter((user) => user.active)
    |> Array.map((user) => user.name)
}
```

## Core forms

- `import` and `export` for modules
- `name = expr` for bindings
- `type` declarations for named types
- arrow functions only
- records and arrays as plain immutable values
- `if`, `match`, `try`, `catch`, and `throw`
- `yield` inside generator arrows
- `defer` for explicit laziness


---
title: Syntax Overview
description: A quick tour of the main FScript syntax forms before diving into each feature in detail.
---

FScript uses a small, expression-oriented syntax with arrows, immutable bindings, records, arrays, pattern matching, and explicit imports.

## A compact tour

```fscript
import Array from 'std:array'
import Logger from 'std:logger'

type User = {
  id: String,
  name: String,
  active: Boolean,
}

formatNames = (users: User[]): String[] => users
  |> Array.filter((user) => user.active)
  |> Array.map((user) => user.name)

main = (): Undefined => {
  names = formatNames([
    { id: '1', name: 'Ada', active: true },
    { id: '2', name: 'Grace', active: false },
  ])

  Logger.info(names)
}

main()
```

## Forms you will use most

- `import` and `export` for modules
- `name = expression` for immutable bindings
- `type Name = ...` for type aliases
- `(args): Return => expr` for functions
- `{ ... }` for records and block expressions
- `[ ... ]` for arrays
- `if (...) { ... } else { ... }` for conditional expressions
- `match (value) { ... }` for pattern-based branching
- `|>` for pipe-based composition

## Major differences from JavaScript or TypeScript

- no `function` keyword
- no `return`
- no `let`, `const`, or `var`
- no class syntax
- no array or object prototype methods
- no `async` / `await`

## Reading the rest of the guide

If you are new to FScript, a useful order is:

1. [Modules](./modules.md)
2. [Bindings and Immutability](./bindings-and-immutability.md)
3. [Functions](./functions.md)
4. [Records and Arrays](./records-and-arrays.md)
5. [Control Flow](./control-flow.md)
6. [Pipes](./pipes.md)

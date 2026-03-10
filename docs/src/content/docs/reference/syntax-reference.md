---
title: Syntax Reference
description: A quick reference for the main FScript syntax forms.
---

# Syntax Reference

Use this page when you want a compact reminder rather than a tutorial.

## Modules

```fscript
import Array from 'std:array'
import { parseUser } from './user.fs'

export readUser = (path: String): User => parseUser(path)
export type User = { id: String, name: String }
```

## Bindings

```fscript
answer = 42
{ name } = user
[first, second] = items
```

## Functions

```fscript
add = (a: Number, b: Number): Number => a + b
```

## Types

```fscript
type User = { id: String, name: String }
type Maybe<T> = T | Null
```

## Control flow

```fscript
if (active) { 'yes' } else { 'no' }

match (value) {
  'ok' => 'done'
  'error' => 'failed'
}
```

## Other important reminders

- blocks return their final expression
- `return` is not part of Draft 0.1
- arrays and records are immutable
- generator arrows use `*() => { ... }`
- `defer expr` creates explicit laziness

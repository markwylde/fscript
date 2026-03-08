---
title: Syntax Reference
description: A quick reference for the main FScript syntax forms.
---

# Syntax Reference

Use this page when you want a compact reminder rather than a tutorial.

## Modules

```fs
import Array from 'std:array'
import { parseUser } from './user.fs'

export readUser = (path: String): User => parseUser(path)
export type User = { id: String, name: String }
```

## Bindings

```fs
answer = 42
{ name } = user
[first, second] = items
```

## Functions

```fs
add = (a: Number, b: Number): Number => a + b
```

## Types

```fs
type User = { id: String, name: String }
type Maybe<T> = T | Null
```

## Control Flow

```fs
if (active) { 'yes' } else { 'no' }

match (value) {
  'ok' => 'done'
  'error' => 'failed'
}
```

## Related Pages

- [Grammar reference](./grammar-reference.md)
- [Keywords](./keywords.md)
- [Operators](./operators.md)

---
title: Bindings and Immutability
description: Immutable bindings, block scope, and how FScript models updates by creating new values.
---

Bindings use plain `=` and are immutable.

## Basic bindings

```fscript
answer = 42
name = 'Ada'
```

Rules:

- bindings are block-scoped
- rebinding the same name in the same scope is a compile error
- shadowing in a nested inner scope is allowed
- `let`, `const`, and `var` are not part of the language

## Immutability is not just for names

Records and arrays are also immutable in Draft 0.1.

These are invalid:

```fscript
user.name = 'Grace'
items[0] = 10
```

Instead, create new values:

```fscript
import Object from 'std:object'

nextUser = Object.spread(user, { name: 'Grace' })
```

## Why this matters

Immutability makes code easier to reason about and supports stronger compiler guarantees. It also fits naturally with FScript's pipe-oriented and expression-oriented style.

## Comparison to JavaScript or TypeScript

If you are used to mutable local variables, the main habit change is to model steps as new values:

```fscript
trimmed = String.trim(text)
normalized = String.lowercase(trimmed)
```

That style is often clearer anyway because intermediate states are named explicitly.

---
title: Bindings and Immutability
description: Immutable bindings, block scope, and how FScript models updates by creating new values.
---

Bindings use plain `=` and are immutable.

```fs
answer = 42
name = 'Ada'
```

## Rules

- bindings are immutable
- bindings are block-scoped
- rebinding the same name in the same scope is a compile error
- nested shadowing is allowed
- `let`, `const`, and `var` are not supported

Records and arrays are also immutable:

```fs
user.name = 'Grace' // invalid
items[0] = 10 // invalid
```

Instead, create new values:

```fs
import Object from 'std:object'

next = Object.spread(base, { active: true })
```


---
title: Functions
description: Arrow-only functions, closures, parameter annotations, and the expression-oriented function model.
---

Functions are written with arrow syntax only.

```fs
add = (a: Number, b: Number): Number => a + b
```

## Rules

- The `function` keyword is not supported.
- Functions are first-class values.
- Closures are supported.
- Functions may be pure or effectful.
- The compiler infers whether a function is pure or effectful.

## Block-bodied functions

```fs
greet = (name: String): String => {
  'hello ' + name
}
```

The final expression in the block becomes the result.


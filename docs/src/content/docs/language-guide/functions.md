---
title: Functions
description: Arrow-only functions, closures, parameter annotations, and the expression-oriented function model.
---

Functions are written with arrow syntax only.

```fscript
add = (a: Number, b: Number): Number => a + b
```

## Core rules

- the `function` keyword is not supported
- functions are first-class values
- closures are supported
- functions may be pure or effectful
- the compiler infers whether a function is pure or effectful

## Block-bodied functions

```fscript
greet = (name: String): String => {
  message = 'hello ' + name
  message
}
```

The final expression in the block becomes the result. There is no `return` keyword.

## Closures

```fscript
makeAdder = (x: Number) => (y: Number): Number => x + y

add5 = makeAdder(5)
result = add5(3)
```

## Practical note

Draft 0.1 expects explicit parameter annotations in many places, especially for public APIs. Local return types may often be inferred, but exported functions should usually keep return types explicit.

## Comparison to JavaScript

If you already prefer arrow functions in JS or TS, the surface will feel familiar. The bigger shift is that arrows are not one option among many; they are the whole function model.

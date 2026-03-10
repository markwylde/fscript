---
title: Currying and Partial Application
description: Why every multi-parameter function is curried and how partial application becomes a default workflow.
---

Every multi-parameter function is curried automatically.

## Equivalent forms

These mean the same thing:

```fscript
add = (a: Number, b: Number): Number => a + b
```

```fscript
add = (a: Number) => (b: Number): Number => a + b
```

## Partial application is always available

```fscript
add10 = add(10)
result = add10(5)
```

This is why the standard library is designed around data-last helpers. It makes small reusable pipeline stages easy to build.

## Example with arrays

```fscript
import Array from 'std:array'

onlyActive = Array.filter((user) => user.active)
namesOnly = Array.map((user) => user.name)
```

Each value above is a reusable function produced through partial application.

## Comparison to JavaScript or TypeScript

Currying exists in JS and TS libraries, but it is not the default language model. In FScript it is built into how functions work, so API design and everyday usage both lean on it heavily.

## Rule to remember

Calling a function with too many arguments is a type error in Draft 0.1.

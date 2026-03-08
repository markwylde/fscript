---
title: Currying and Partial Application
description: Why every multi-parameter function is curried and how partial application becomes a default workflow.
---

Every multi-parameter function is curried automatically.

```fs
add = (a: Number, b: Number): Number => a + b
```

This is semantically equivalent to:

```fs
add = (a: Number) => (b: Number): Number => a + b
```

## Partial application

```fs
add10 = add(10)
value = add10(5)
```

## Important rule

Calling a function with fewer arguments returns another function. Calling with too many arguments is a type error in Draft 0.1.


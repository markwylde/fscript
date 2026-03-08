---
title: Function Types
description: Curried function types, partial application, and parameter or result typing in FScript.
---

Functions are first-class and curried by default.

```fs
add = (a: Number, b: Number): Number => a + b
```

Semantically, that is:

```fs
Number -> Number -> Number
```

## Rules

- all multi-parameter functions are curried
- partial application is valid when argument types line up
- too many arguments is a type error


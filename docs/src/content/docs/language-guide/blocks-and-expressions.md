---
title: Blocks and Expressions
description: Expression-oriented blocks, implicit final values, and why FScript does not use return.
---

FScript is expression-oriented. Blocks are not just statement containers; they evaluate to values.

## Simple example

```fscript
result = {
  a = 1
  b = 2
  a + b
}
```

`result` becomes `3` because the final expression in the block is `a + b`.

## Function bodies work the same way

```fscript
double = (value: Number): Number => {
  next = value * 2
  next
}
```

There is no `return` keyword in Draft 0.1.

## Why this style is useful

- small helpers stay compact
- intermediate values can still be named
- control flow like `if`, `match`, and `try/catch` fits naturally because they also produce values

## Comparison to JavaScript

JavaScript mixes expressions and statements heavily. FScript keeps more forms expression-oriented so composition stays uniform.

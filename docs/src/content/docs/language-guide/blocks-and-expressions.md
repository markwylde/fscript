---
title: Blocks and Expressions
description: Expression-oriented blocks, implicit final values, and why FScript does not use return.
---

FScript is expression-oriented. Blocks are not just statement containers; they evaluate to values.

```fs
compute = (): Number => {
  a = 1
  b = 2
  a + b
}
```

## Rules

- the final expression in a block becomes the block value
- there is no `return` keyword in Draft 0.1
- local bindings inside blocks are still immutable

This style keeps control flow and value flow tightly connected.


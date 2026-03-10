---
title: Literal Types
description: Narrow value-level literals that support precise tags, domain states, and exhaustive matching.
---

Literal values can participate in narrower types.

## Examples

```fscript
status = 'ok' // 'ok'
type Status = 'ok' | 'error'
```

## Why literal types matter

- they make tagged unions precise
- they help exhaustiveness
- they model small closed domains cleanly

## Comparison to TypeScript

If you already use string literal unions in TypeScript, the idea is the same. In FScript they are especially central because tagged unions are the preferred sum-type model.

---
title: Union Types
description: Model values that may be one of several alternatives and narrow them through control flow and match.
---

Union types represent values that may be one of several alternatives.

## Example

```fscript
type Id = Number | String
```

## When to use unions

- optional-like domain values
- parser outputs before narrowing
- tagged union variant groups

## How unions become useful

Unions are usually paired with:

- `if` checks
- `match`
- tagged discriminants

Those tools narrow the active case so the rest of the code can stay precise.

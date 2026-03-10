---
title: Narrowing and Exhaustiveness
description: How control flow and pattern matching refine types and why tagged unions should be handled exhaustively.
---

Pattern matching narrows types within each branch.

## Example

```fscript
type LoadResult =
  | { tag: 'loading' }
  | { tag: 'loaded', value: String }
  | { tag: 'failed', message: String }

message = (result: LoadResult): String => match (result) {
  { tag: 'loading' } => 'loading'
  { tag: 'loaded', value } => value
  { tag: 'failed', message } => message
}
```

Inside each branch, the active variant is known more precisely.

## Why exhaustiveness matters

Exhaustive handling helps:

- avoid forgotten cases
- keep union-heavy code maintainable
- support stronger compiler guarantees

## Comparison to TypeScript

TypeScript can narrow through control flow too, but FScript leans harder on `match` and tagged unions as the standard way to express those checks.

---
title: Pattern Matching
description: Match on literals, records, arrays, and tagged unions with exhaustive branch coverage.
---

`match` is the preferred branching tool for tagged unions and other value-shape decisions.

## Simple example

```fscript
describe = (value: Number | String): String => match (value) {
  0 => 'zero'
  'ok' => 'status ok'
  _ => 'something else'
}
```

## Tagged union example

```fscript
type User =
  | { tag: 'guest' }
  | { tag: 'member', name: String }

greet = (user: User): String => match (user) {
  { tag: 'guest' } => 'hello guest'
  { tag: 'member', name } => 'hello ' + name
}
```

## Why `match` matters

- it makes value shape explicit
- it narrows types inside each arm
- it encourages exhaustive handling of tagged unions

## Comparison to TypeScript

TypeScript often uses `switch` plus ad hoc narrowing checks. FScript gives pattern matching a more central role, which tends to make union-heavy code shorter and clearer.

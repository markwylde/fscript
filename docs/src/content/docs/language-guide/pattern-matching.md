---
title: Pattern Matching
description: Match on literals, records, arrays, and tagged unions with exhaustive branch coverage.
---

`match` is the preferred branching tool for tagged unions and other value-shape decisions.

## Tagged union example

```fs
type User =
  | { tag: 'guest' }
  | { tag: 'member', id: String, name: String }

describe = (user: User): String => {
  match (user) {
    { tag: 'guest' } => 'guest'
    { tag: 'member', name } => 'member: ' + name
  }
}
```

## Benefits

- branch-local type narrowing
- readable destructuring
- exhaustiveness over tagged unions


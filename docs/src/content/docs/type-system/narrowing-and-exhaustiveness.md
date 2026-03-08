---
title: Narrowing and Exhaustiveness
description: How control flow and pattern matching refine types and why tagged unions should be handled exhaustively.
---

Pattern matching narrows types within each branch.

```fs
match (user) {
  { tag: 'guest' } => 'guest'
  { tag: 'member', name } => name
}
```

For tagged unions, `match` should be exhaustive in Draft 0.1. This keeps control flow explicit and avoids silent fallthrough.


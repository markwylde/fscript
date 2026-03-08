---
title: Tagged Unions
description: The preferred sum-type model in FScript, built from plain records with a stable discriminant field.
---

Tagged unions are the preferred way to model alternatives in FScript.

```fs
type User =
  | { tag: 'guest' }
  | { tag: 'member', id: String, name: String }
```

## Why they work well

- plain data
- structural typing
- natural fit for `match`
- branch-local narrowing
- exhaustiveness checks


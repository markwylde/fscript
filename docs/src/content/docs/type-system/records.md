---
title: Record Types
description: Structurally typed immutable records with closed-by-default shape expectations in Draft 0.1.
---

Records are structurally typed immutable data.

```fs
type User = {
  id: String,
  name: String,
  active: Boolean,
}
```

## Rules

- field names and field types must match
- field order does not matter
- record values are immutable
- Draft 0.1 recommends closed shapes by default


---
title: Record Types
description: Structurally typed immutable records with closed-by-default shape expectations in Draft 0.1.
---

Records are structurally typed immutable data.

## Example

```fscript
type User = {
  id: String,
  name: String,
  active: Boolean,
}
```

## Rules

- compatibility is structural
- field order does not matter
- field names and field types must match
- record values are immutable

## Draft 0.1 recommendation

Record shapes should be treated as closed by default. Unknown extra fields should not be silently accepted unless the language explicitly grows that capability later.

## Why that matters

Closed-by-default expectations help:

- soundness
- diagnostics
- optimization

## Comparison to TypeScript

TypeScript's structural typing will feel familiar, but FScript is aiming for a tighter and more predictable shape model.

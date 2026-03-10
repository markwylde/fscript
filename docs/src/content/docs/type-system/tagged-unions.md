---
title: Tagged Unions
description: The preferred sum-type model in FScript, built from plain records with a stable discriminant field.
---

Tagged unions are the preferred way to model alternatives in FScript.

## Example

```fscript
type User =
  | { tag: 'guest' }
  | { tag: 'member', id: String, name: String }
```

## Why they are preferred

- they stay within the plain-record structural model
- `match` can narrow them clearly
- exhaustiveness is easier to reason about

## Good convention

Use a stable discriminant field such as `tag` with literal values.

## Comparison to TypeScript

This is close to discriminated unions in TypeScript, but it is even more central to the FScript style because there are no enums or classes to compete with the pattern.

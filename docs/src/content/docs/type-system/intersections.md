---
title: Intersection Types
description: Combine compatible type requirements into a single value shape.
---

Intersection types represent values that satisfy multiple type requirements at once.

## Example

```fscript
type Named = { name: String }
type Active = { active: Boolean }
type ActiveNamed = Named & Active
```

## When they help

Intersections are useful when you want to express combined structural requirements without inventing a whole new base type just for composition.

## Practical note

As with the rest of Draft 0.1, the type system aims for predictable composition rather than every edge case of TypeScript's more permissive behavior.

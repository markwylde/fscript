---
title: Operators
description: The small set of operators and operator-like forms highlighted in the docs.
---

# Operators

FScript keeps its operator surface relatively small and leans heavily on function composition.

## Common Forms

- `=` for immutable bindings
- `+` for numeric or string combination where valid
- `|>` for pipe composition
- `|` for union types
- `&` for intersection types
- `=>` for arrow functions and match arms

## Guidance

When a transformation can be expressed clearly as a function call, FScript usually prefers that over a larger set of special operators.

## Related Pages

- [Pipes](../language-guide/pipes.md)
- [Union types](../type-system/unions.md)
- [Intersection types](../type-system/intersections.md)

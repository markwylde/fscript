---
title: Values and Equality
description: Runtime value categories, immutable data, and structural equality rules.
---

# Values and Equality

The runtime needs a concrete value model for both execution and interop boundaries.

## Runtime Value Categories

- number
- string
- boolean
- null
- undefined
- record
- array
- function and closure
- generator
- deferred task
- effect task handle
- tagged union record values

## Structural Equality

The runtime must support structural equality for plain data:

- primitives by value
- arrays by element comparison
- records by field comparison
- tagged unions by structural comparison

Functions, generators, and streams are not comparable in the same way.

## Related Pages

- [Records and arrays](../language-guide/records-and-arrays.md)
- [Record types](../type-system/records.md)

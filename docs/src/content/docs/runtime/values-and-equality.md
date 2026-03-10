---
title: Values and Equality
description: Runtime value categories, immutable data, and structural equality rules.
---

# Values and Equality

The runtime needs a concrete model for the values that flow through execution.

## Minimum value categories

- number
- string
- boolean
- null
- undefined
- record
- array
- function or closure
- generator
- deferred task
- effect task handle

## Equality rules

Structural equality applies to plain data:

- primitives compare by value
- arrays compare element by element
- records compare field by field
- tagged unions compare structurally

The runtime should not treat functions or generators as structurally comparable values.

## Why this matters

These rules help keep FScript's immutable data model useful and predictable without turning every runtime value into a deep-equality free-for-all.

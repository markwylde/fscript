---
title: Primitive Types
description: Number, String, Boolean, Null, Undefined, Never, and Unknown in the FScript type system.
---

Draft 0.1 recommends these canonical built-in names:

- `Number`
- `String`
- `Boolean`
- `Null`
- `Undefined`
- `Never`
- `Unknown`

## Example values

```fscript
count = 10
name = 'Ada'
active = true
empty = Null
missing = Undefined
```

## Important rules

- there is no implicit coercion between primitive types
- `Null` is not assignable to everything by default
- `Undefined` is not assignable to everything by default

## Comparison to JavaScript

JavaScript performs many implicit coercions. FScript does not. That makes programs more predictable and keeps the type system easier to trust.

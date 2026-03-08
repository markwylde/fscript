---
title: Control Flow
description: Use if expressions, match expressions, and try/catch as value-producing control flow constructs.
---

Control flow in FScript is expression-oriented.

## `if`

```fs
label = if (active) {
  'active'
} else {
  'inactive'
}
```

## `match`

```fs
message = match (status) {
  'ok' => 'good'
  'error' => 'bad'
}
```

## `try` and `catch`

`try` and `catch` are also part of the expression model. They produce values rather than acting like statement-only control flow.


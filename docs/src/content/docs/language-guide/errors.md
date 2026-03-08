---
title: Errors
description: Use Result for expected failures and throw for exceptional control flow with plain-data error values.
---

FScript supports two broad error styles.

## `Result`

Use `Result<T, E>` for expected failure:

```fs
type ParseError = { tag: 'parse_error', message: String }
```

## `throw`

Use `throw` when the control flow is exceptional and you want the nearest `catch` to handle it.

## Important difference from JavaScript

Thrown values are plain data, not error class instances.


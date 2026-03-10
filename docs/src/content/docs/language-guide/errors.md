---
title: Errors
description: Use Result for expected failures and throw for exceptional control flow with plain-data error values.
---

FScript supports two broad error styles.

## `Result` for expected failures

Use `Result<T, E>` when callers should recover intentionally.

```fscript
type Result<T, E> =
  | { tag: 'ok', value: T }
  | { tag: 'error', error: E }
```

This is the preferred model for parsing, validation, and other ordinary failure cases.

## `throw` for exceptional situations

```fscript
fail = (message: String): Never => {
  throw { tag: 'fatal', message }
}
```

`try/catch` can recover from thrown values when needed.

## Good rule of thumb

- use `Result` when failure is part of the domain
- use `throw` when continuing normally is not the expected path

## Comparison to JavaScript

JavaScript code often uses exceptions for many ordinary failure modes. FScript nudges you toward typed data-first failure handling instead.

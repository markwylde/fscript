---
title: std:result
description: Typed expected-failure handling with ok and error variants.
---

# `std:result`

`std:result` is the preferred model for recoverable failures.

```fs
import Result from 'std:result'
```

## Core Type

```fs
type Result<T, E> =
  | { tag: 'ok', value: T }
  | { tag: 'error', error: E }
```

## Representative API

```fs
Result.ok = <T, E>(value: T): Result<T, E>
Result.error = <T, E>(error: E): Result<T, E>
Result.map = <T, U, E>(fn: (value: T): U, result: Result<T, E>): Result<U, E>
Result.andThen = <T, U, E>(fn: (value: T): Result<U, E>, result: Result<T, E>): Result<U, E>
Result.withDefault = <T, E>(fallback: T, result: Result<T, E>): T
Result.isOk = <T, E>(result: Result<T, E>): Boolean
Result.isError = <T, E>(result: Result<T, E>): Boolean
```

## Example

```fs
import Number from 'std:number'
import Result from 'std:result'
import String from 'std:string'

type ParseError = {
  tag: 'parse_error',
  message: String,
}

parsePort = (text: String): Result<Number, ParseError> => {
  if (String.isDigits(text)) {
    Result.ok(Number.parse(text))
  } else {
    Result.error({
      tag: 'parse_error',
      message: 'port must contain digits only',
    })
  }
}
```

## Current Implementation Note

The current runtime-backed implementation already exposes `ok`, `error`, `isOk`, `isError`, and `withDefault`.

## Related Pages

- [Errors](../language-guide/errors.md)
- [Tagged unions](../type-system/tagged-unions.md)

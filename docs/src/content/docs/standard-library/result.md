---
title: std:result
description: Typed expected-failure handling with ok and error variants.
---

# `std:result`

`std:result` is the preferred model for recoverable failures.

```fscript
import Result from 'std:result'
```

## Core type

```fscript
type Result<T, E> =
  | { tag: 'ok', value: T }
  | { tag: 'error', error: E }
```

## Representative API

```fscript
Result.ok = <T, E>(value: T): Result<T, E>
Result.error = <T, E>(error: E): Result<T, E>
Result.map = <T, U, E>(fn: (value: T): U, result: Result<T, E>): Result<U, E>
Result.mapError = <T, E, F>(fn: (error: E): F, result: Result<T, E>): Result<T, F>
Result.andThen = <T, U, E>(fn: (value: T): Result<U, E>, result: Result<T, E>): Result<U, E>
Result.withDefault = <T, E>(fallback: T, result: Result<T, E>): T
Result.isOk = <T, E>(result: Result<T, E>): Boolean
Result.isError = <T, E>(result: Result<T, E>): Boolean
```

## Example

```fscript
parsePort = (text: String) => {
  if (String.isDigits(text)) {
    Result.ok(Number.parse(text))
  } else {
    Result.error({ tag: 'parse_error', message: 'port must contain digits only' })
  }
}
```

## Why use it

- success and failure stay explicit in the type
- callers can recover with `match`
- ordinary failures do not need exceptions

## Current implementation note

The current runtime-backed surface already exposes the core constructors and basic helpers such as `ok`, `error`, `isOk`, `isError`, and `withDefault`.

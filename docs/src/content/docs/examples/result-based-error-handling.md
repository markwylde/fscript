---
title: Result-Based Error Handling
description: Prefer typed recoverable errors over exceptions for expected failures.
---

# Result-Based Error Handling

When a failure is expected and callers should recover from it, `Result<T, E>` is usually the clearest model.

## Example: parse a port

```fscript
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

## Example: compose with `andThen`

```fscript
requireInRange = (value: Number): Result<Number, ParseError> => {
  if (value > 0 && value < 65536) {
    Result.ok(value)
  } else {
    Result.error({
      tag: 'parse_error',
      message: 'port must be between 1 and 65535',
    })
  }
}

parseValidPort = (text: String): Result<Number, ParseError> => {
  parsePort(text)
    |> Result.andThen(requireInRange)
}
```

## Why prefer this over exceptions

- the success and error shapes are part of the type
- callers can handle cases with `match`
- failure stays visible in signatures

Use `throw` for exceptional situations. Use `Result` when failure is part of ordinary control flow.

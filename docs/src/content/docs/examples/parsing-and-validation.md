---
title: Parsing and Validation
description: Parse raw input, validate it explicitly, and return typed results.
---

# Parsing and Validation

One of the most important habits in FScript is keeping boundary validation explicit. Raw input from JSON, files, or other host sources should be parsed and checked before the rest of the program treats it as trusted data.

## Example

```fscript
import Json from 'std:json'
import Result from 'std:result'
import String from 'std:string'

type User = {
  id: String,
  name: String,
}

type ParseError = {
  tag: 'parse_error',
  message: String,
}

parseUser = (text: String): Result<User, ParseError> => {
  parsed = Json.parse(text)

  match (parsed) {
    { tag: 'ok', value: { id, name } } => Result.ok({ id, name })
    { tag: 'ok' } => Result.error({
      tag: 'parse_error',
      message: 'expected an object with id and name',
    })
    { tag: 'error', error } => Result.error({
      tag: 'parse_error',
      message: error.message,
    })
  }
}
```

## Why this style matters

The type system aims to eliminate internal mismatches in well-typed code, but host boundaries still need validation. That is why parsing and validation stay visible in the language model instead of being hidden behind unchecked casts.

## Comparison to TypeScript

In TypeScript it is common to write `const user = JSON.parse(text) as User`. FScript pushes you toward a safer pattern: parse first, validate shape explicitly, then return `Result<User, E>`.

## Good practice

- keep raw boundary values in a separate phase
- convert them into domain types explicitly
- use tagged errors so callers can recover intentionally

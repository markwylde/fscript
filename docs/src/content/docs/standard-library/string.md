---
title: std:string
description: String transformation and inspection helpers.
---

# `std:string`

`std:string` groups common string helpers under an explicit import.

```fs
import String from 'std:string'
```

## Representative API

```fs
String.length = (value: String): Number
String.uppercase = (value: String): String
String.lowercase = (value: String): String
String.trim = (value: String): String
String.split = (separator: String, value: String): String[]
String.join = (separator: String, values: String[]): String
String.isDigits = (value: String): Boolean
```

## Example

```fs
import String from 'std:string'

normalized = String.uppercase(String.trim('  ada  '))
```

## Current Implementation Note

The current runtime-backed implementation already exposes `trim`, `uppercase`, `lowercase`, and `isDigits`.

## Related Pages

- [Primitive types](../type-system/primitive-types.md)
- [Result-based error handling example](../examples/result-based-error-handling.md)

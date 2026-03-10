---
title: std:string
description: String transformation and inspection helpers.
---

# `std:string`

`std:string` provides explicit string helpers.

```fscript
import String from 'std:string'
```

## Representative API

```fscript
String.length = (value: String): Number
String.uppercase = (value: String): String
String.lowercase = (value: String): String
String.trim = (value: String): String
String.split = (separator: String, value: String): String[]
String.join = (separator: String, values: String[]): String
String.startsWith = (prefix: String, value: String): Boolean
String.endsWith = (suffix: String, value: String): Boolean
String.contains = (part: String, value: String): Boolean
String.isDigits = (value: String): Boolean
```

## Example

```fscript
normalized = text
  |> String.trim
  |> String.lowercase
```

## Comparison to JavaScript

Instead of instance methods like `text.trim()`, FScript uses imported helpers so the standard library stays explicit and uniform.

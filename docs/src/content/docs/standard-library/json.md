---
title: std:json
description: JSON parsing, serialization, and explicit host-boundary typing.
---

# `std:json`

`std:json` handles JSON parsing and serialization while keeping the boundary to typed program data explicit.

```fs
import Json from 'std:json'
```

## Representative API

```fs
Json.jsonToObject = (text: String): Unknown
Json.jsonToString = (value: Unknown): String
Json.jsonToPrettyString = (value: Unknown): String
Json.decode = <T>(decoder: Decoder<T>, value: Unknown): Result<T, DecodeError>
Json.parseAs = <T>(decoder: Decoder<T>, text: String): Result<T, DecodeError>
```

## Why `Unknown` Matters

`Json.jsonToObject` should not pretend arbitrary JSON is already typed FScript data. Decoding is the point where unknown external data becomes validated program data.

## Comment-Tolerant Parsing

`Json.jsonToObject` and `Json.parseAs` accept a relaxed JSON mode for human-edited config files.

Outside string literals, the parser should ignore:

- `//` line comments
- `#` line comments
- `/* ... */` block comments
- lines whose trimmed contents are exactly `---`

This keeps ordinary JSON structure rules while making configuration files easier to maintain.

```fs
import Json from 'std:json'

config = Json.jsonToObject(
  '
  ---
  {
    // service name
    "name": "demo",
    # port for local development
    "port": 8080,
    /* enable extra output */
    "debug": true
  }
  '
)
```

## Compact and Pretty Output

- `Json.jsonToString` produces compact single-line JSON
- `Json.jsonToPrettyString` produces stable multi-line JSON with two-space indentation

```fs
import Json from 'std:json'

value = {
  name: 'demo',
  port: 8080,
}

compact = Json.jsonToString(value)
pretty = Json.jsonToPrettyString(value)
```

## Current Implementation Note

The current runtime-backed implementation exposes `jsonToObject`, `jsonToString`, and `jsonToPrettyString`, and the older `parse` and `stringify` names remain available as compatibility aliases. Decoder-based helpers are still part of the broader planned API shape.

## Related Pages

- [Unknown, Never, Null, and Undefined](../type-system/unknown-never-null-undefined.md)
- [std:logger](./logger.md)
- [Filesystem](./filesystem.md)

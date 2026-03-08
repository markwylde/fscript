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
Json.parse = (text: String): Unknown
Json.stringify = (value: Unknown): String
Json.decode = <T>(decoder: Decoder<T>, value: Unknown): Result<T, DecodeError>
Json.parseAs = <T>(decoder: Decoder<T>, text: String): Result<T, DecodeError>
```

## Why `Unknown` Matters

`Json.parse` should not pretend arbitrary JSON is already typed FScript data. Decoding is the point where unknown external data becomes validated program data.

## Current Implementation Note

The current runtime-backed implementation already exposes `parse` and `stringify`. Decoder-based helpers are part of the broader planned API shape.

## Related Pages

- [Unknown, Never, Null, and Undefined](../type-system/unknown-never-null-undefined.md)
- [Filesystem](./filesystem.md)

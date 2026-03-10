---
title: std:json
description: JSON parsing, serialization, and explicit host-boundary typing.
---

# `std:json`

`std:json` handles parsing and serializing JSON at a host boundary.

```fscript
import Json from 'std:json'
```

## Typical role

JSON is one of the clearest places where runtime data and static types meet. The standard workflow is:

1. read raw text
2. parse JSON
3. validate shape explicitly
4. return a typed domain value

## Example

```fscript
readConfig = (path: String) => {
  text = FileSystem.readFile(path)
  Json.parse(text)
}
```

Then handle the parsed value with `match` or `Result`-based validation instead of assuming it already has the domain shape you want.

## Why the docs emphasize validation

The type system aims to keep well-typed internal code safe, but external data still needs checking. `std:json` is therefore more than a convenience module; it is a reminder that host boundaries stay explicit.

## Current implementation note

The current runtime already ships `std:json`, and the broader docs/examples assume it as part of the available `run` path.

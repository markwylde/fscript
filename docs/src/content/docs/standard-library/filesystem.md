---
title: std:filesystem
description: Runtime-backed file IO and other host filesystem capabilities.
---

# `std:filesystem`

`std:filesystem` provides native file IO through the runtime.

```fs
import FileSystem from 'std:filesystem'
```

## Representative API

```fs
FileSystem.readFile = (path: String): String
FileSystem.writeFile = (path: String, content: String): Undefined
FileSystem.exists = (path: String): Boolean
FileSystem.deleteFile = (path: String): Undefined
FileSystem.readDir = (path: String): String[]
```

## Semantics

- these functions are effectful
- effectful calls start eagerly by default
- they participate in implicit suspension and resolution semantics
- they are provided by the FScript runtime, not Node.js globals

## Example

```fs
import FileSystem from 'std:filesystem'

loadText = (path: String): String => FileSystem.readFile(path)
```

## Current Implementation Note

The current runtime-backed implementation already exposes `readFile`, `writeFile`, `exists`, `deleteFile`, and `readDir`.

## Related Pages

- [Effects](../language-guide/effects.md)
- [Runtime boundaries](../runtime/errors-and-boundaries.md)

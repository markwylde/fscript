---
title: Filesystem Scripts
description: Work with runtime-backed filesystem capabilities through explicit std modules.
---

# Filesystem Scripts

FScript exposes file IO through `std:filesystem`, not through Node.js globals.

```fs
import FileSystem from 'std:filesystem'

loadText = (path: String): String => FileSystem.readFile(path)
```

## Why This Pattern Matters

- the capability is explicit in the import
- the call is effectful by language semantics
- the runtime handles the host boundary

## Related Pages

- [std:filesystem](../standard-library/filesystem.md)
- [Runtime boundaries](../runtime/errors-and-boundaries.md)

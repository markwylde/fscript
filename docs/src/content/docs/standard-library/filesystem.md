---
title: std:filesystem
description: Runtime-backed file IO and other host filesystem capabilities.
---

# `std:filesystem`

`std:filesystem` provides runtime-backed file access.

```fscript
import FileSystem from 'std:filesystem'
```

## Typical usage

```fscript
readConfig = (path: String): String => {
  FileSystem.readFile(path)
}
```

## What this module represents

Filesystem access is effectful. FScript keeps that explicit:

- the capability is imported
- the runtime owns the host interaction
- your code can keep the pure transformation steps separate

## Good practice

- keep file reads and writes near the boundary of the program
- parse and validate file contents explicitly after reading
- move pure shaping work into separate helpers

## Current implementation note

The current runtime already ships filesystem support and uses it in the interpreter-backed `run` path.

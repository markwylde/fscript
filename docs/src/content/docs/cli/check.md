---
title: fscript check
description: Validate and typecheck an FScript source file.
---

# `fscript check`

`check` validates a source file without running it.

## Usage

```sh
fscript check path/to/file.fs
```

## What It Does

The CLI describes this command as:

```text
Typecheck and validate a source file
```

In the current implementation, this includes the frontend validation path for the supported language surface.

## Example

```sh
fscript check examples/hello_world.fs
```

## Related Pages

- [CLI overview](./overview.md)
- [Run](./run.md)
- [Implementation status](../implementation-status/supported-features.md)

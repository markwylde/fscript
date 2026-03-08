---
title: Errors and Boundaries
description: Runtime boundaries, host capabilities, and where validation still matters.
---

# Errors and Boundaries

Well-typed FScript code aims to avoid internal runtime type errors, but host boundaries still matter.

## Typical Boundary Points

- file input
- JSON parsing
- decoding external data
- future native interop boundaries

## Practical Guidance

- prefer `Result<T, E>` for expected failures
- treat JSON and filesystem work as runtime boundaries
- do not assume external data is already typed program data

## Related Pages

- [Errors](../language-guide/errors.md)
- [std:json](../standard-library/json.md)
- [std:filesystem](../standard-library/filesystem.md)

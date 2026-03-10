---
title: Examples Overview
description: Practical example categories drawn from the repository and the language model.
---

# Examples Overview

The examples section shows how the language pieces fit together in realistic shapes rather than isolated syntax snippets.

## What these examples emphasize

- immutable data flow
- explicit `std:` imports
- tagged unions and `Result` for clear control flow
- pipes and currying for composition
- a separation between pure transformation and effectful boundaries

## Suggested reading order

1. [Data Pipelines](./data-pipelines.md)
2. [Parsing and Validation](./parsing-and-validation.md)
3. [Result-Based Error Handling](./result-based-error-handling.md)
4. [Filesystem Scripts](./filesystem-scripts.md)
5. [Generators and Sequences](./generators-and-sequences.md)

## How to read the examples

Most examples deliberately look a little more explicit than equivalent JavaScript or TypeScript code. That is part of the language model:

- array helpers are imported rather than methods
- data is rebuilt rather than mutated
- parsing and validation stay visible instead of being hidden inside loose casts

## Current implementation note

These pages are written against the Draft 0.1 language model and the current shipped CLI/runtime surface. Where a page touches implementation maturity, it calls that out directly instead of quietly assuming full parity everywhere.

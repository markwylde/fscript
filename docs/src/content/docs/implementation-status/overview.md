---
title: Implementation Status
description: How to read the docs when the language and implementation are still Draft 0.1.
---

# Implementation Status

FScript is documented from both specification and current implementation sources. Those are related, but they are not identical.

## How To Read These Docs

- "FScript specifies ..." refers to the Draft 0.1 language design
- "The current implementation ..." refers to behavior present in the repository today
- "Draft 0.1 plans ..." refers to intended future direction within the current design

## Practical Reality

- the specs are broader than some currently shipped runtime surfaces
- `run` is currently broader than `compile`
- core stdlib modules exist, but some expose a smaller implementation subset than their representative spec APIs

## Related Pages

- [Supported features](./supported-features.md)
- [Compile vs run](./compile-vs-run.md)
- [Roadmap](./roadmap.md)

---
title: Module Loading
description: How FScript loads modules, initializes them once, and handles cycles.
---

# Module Loading

Each `.fs` file is a module, and top-level module code executes once when the module is loaded.

## Key Rules

- module resolution is defined by the compiler and runtime
- `std:` modules are provided by the language runtime
- user modules can import relative `.fs` files
- Draft 0.1 treats circular imports as a compile error

## Current Implementation Notes

The implementation plan already calls out runtime support for:

- canonical path resolution
- cycle rejection
- once-per-module initialization

## Related Pages

- [Modules](../language-guide/modules.md)
- [CLI run](../cli/run.md)

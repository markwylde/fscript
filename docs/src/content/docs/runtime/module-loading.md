---
title: Module Loading
description: How FScript loads modules, initializes them once, and handles cycles.
---

# Module Loading

Each `.fs` file is a module, and top-level code runs once when the module is loaded.

## Key rules

- modules initialize once
- user-module imports are resolved through the FScript toolchain
- circular imports are a compile error in Draft 0.1
- standard-library modules come from runtime-backed `std:` implementations

## Current implementation note

The current runtime already supports canonical path resolution, cycle rejection, and once-per-module initialization for user imports.

## Why this matters

Predictable module loading makes both the typechecker and the runtime easier to reason about than looser JavaScript environments with multiple interop models.

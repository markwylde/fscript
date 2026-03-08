---
title: Runtime Overview
description: The native runtime model behind FScript's pure expressions, effects, tasks, and module loading.
---

# Runtime Overview

FScript is designed around a native runtime rather than a JavaScript engine. The runtime exists to execute well-typed programs predictably and efficiently while preserving the language's async-by-semantics model.

## Runtime Responsibilities

- program startup
- module initialization
- execution of effectful tasks
- implicit suspension and resumption
- generator state management
- native implementations of `std:` modules
- error propagation across runtime boundaries

## Runtime Principles

- pure code should not pay scheduler overhead
- effectful calls start eagerly by default
- laziness is explicit through `defer`
- observable ordering is preserved unless independence is clear

## Read More

- [Execution model](./execution-model.md)
- [Scheduler](./scheduler.md)
- [Tasks](./tasks.md)
- [Module loading](./module-loading.md)

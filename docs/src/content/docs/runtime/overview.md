---
title: Runtime Overview
description: The native runtime model behind FScript's pure expressions, effects, tasks, and module loading.
---

# Runtime Overview

The runtime exists to execute well-typed FScript programs efficiently and predictably without depending on a JavaScript engine.

## Core responsibilities

- program startup
- module initialization
- execution of effectful tasks
- implicit suspension and resumption
- generator state management
- native implementations of `std:` modules
- error propagation across host boundaries

## Core principles

- pure code should not pay async scheduler overhead
- effectful work may suspend implicitly
- effectful calls start eagerly by default
- `defer` is the explicit tool for laziness

## What this means in practice

FScript source often looks sequential even when effectful work is involved. The runtime, not `Promise` syntax in user code, coordinates when work starts and when execution needs to wait for results.

## Related pages

- [Execution Model](./execution-model.md)
- [Scheduler](./scheduler.md)
- [Tasks](./tasks.md)
- [Module Loading](./module-loading.md)

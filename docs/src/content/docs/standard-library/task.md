---
title: std:task
description: Runtime task helpers and explicit control over deferred work.
---

# `std:task`

`std:task` exposes runtime helpers for task and deferred-work coordination.

```fscript
import Task from 'std:task'
```

## Current runtime-backed helpers

The implementation plan already calls out helpers such as:

- `Task.all`
- `Task.race`
- `Task.spawn`
- `Task.defer`
- `Task.force`

## Why this module exists

FScript handles many effect details in the runtime automatically, but some workflows still benefit from explicit task-oriented helpers. This module is the place for those controls rather than exposing a JavaScript-style `Promise` API in user code.

## Important context

Task behavior is tied closely to the current runtime and scheduler implementation, so this page should be read together with the runtime docs.

## Related pages

- [Execution Model](../runtime/execution-model.md)
- [Scheduler](../runtime/scheduler.md)
- [Tasks](../runtime/tasks.md)

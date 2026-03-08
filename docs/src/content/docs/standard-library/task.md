---
title: std:task
description: Runtime task helpers and explicit control over deferred work.
---

# `std:task`

`std:task` exposes runtime task concepts in a more explicit library surface.

```fs
import Task from 'std:task'
```

## Representative API

```fs
Task.all = <T>(tasks: Task<T>[]): T[]
Task.race = <T>(tasks: Task<T>[]): T
Task.spawn = <T>(task: Task<T>): Task<T>
Task.force = <T>(deferred: Deferred<T>): T
Task.defer = <T>(fn: (): T): Deferred<T>
```

## Guidance

The language already has native `defer`, so this module should stay small and focused. It is most useful where explicit control is clearer than relying only on implicit scheduling.

## Current Implementation Note

The current runtime-backed implementation already exposes `all`, `defer`, and `force`.

## Related Pages

- [Defer and laziness](../language-guide/defer-and-laziness.md)
- [Runtime tasks](../runtime/tasks.md)

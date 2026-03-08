---
title: Tasks
description: Eager tasks, deferred tasks, and the runtime representation of effectful work.
---

# Tasks

The runtime represents effectful work explicitly.

## Minimum Task States

- created
- ready
- running
- waiting
- completed
- failed
- canceled reserved for future use

## Core Categories

- eager tasks start when execution reaches them
- deferred tasks are created by `defer` and start only when forced or invoked

## Practical Rule

Pure functions do not become scheduler-managed tasks. Only effectful operations need that machinery.

## Related Pages

- [Defer and laziness](../language-guide/defer-and-laziness.md)
- [std:task](../standard-library/task.md)
- [Scheduler](./scheduler.md)

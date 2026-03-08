---
title: Scheduler
description: The single-threaded scheduler that coordinates effectful work in Draft 0.1.
---

# Scheduler

Draft 0.1 uses a single-threaded scheduler.

## Why Single-Threaded

- simpler semantics
- easier determinism
- lower implementation complexity
- closer alignment with source-order effect rules

## Responsibilities

- manage ready tasks
- manage suspended tasks
- resume tasks when dependencies resolve
- preserve observable ordering
- support explicit deferred tasks

## Important Caveat

The runtime spec describes the intended scheduler model clearly, but implementation parity is still evolving. The docs keep that distinction visible so the conceptual model stays accurate without overstating what is complete today.

## Related Pages

- [Execution model](./execution-model.md)
- [Tasks](./tasks.md)
- [Implementation status](../implementation-status/supported-features.md)

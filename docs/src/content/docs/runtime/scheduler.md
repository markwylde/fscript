---
title: Scheduler
description: The single-threaded scheduler that coordinates effectful work in Draft 0.1.
---

# Scheduler

Draft 0.1 uses a single-threaded scheduler.

## Why single-threaded

- simpler semantics
- easier determinism
- lower implementation complexity
- closer alignment with source-order effect rules

## Responsibilities

- manage ready tasks
- manage suspended tasks
- resume work when dependencies resolve
- preserve observable ordering for effects
- support explicit deferred tasks

## Current implementation note

The repository already has a shared scheduler abstraction used by the runtime and interpreter for deferred execution and ordinary effectful native calls. A remaining roadmap item is broader long-lived dependency draining across a whole evaluation.

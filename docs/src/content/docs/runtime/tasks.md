---
title: Tasks
description: Eager tasks, deferred tasks, and the runtime representation of effectful work.
---

# Tasks

The runtime represents effectful work explicitly as tasks.

## Minimum task states

- created
- ready
- running
- waiting
- completed
- failed
- canceled

## Two main categories

`eager task`
: starts when execution reaches it

`deferred task`
: created through `defer` and started only when forced

## Why tasks matter

Tasks are how the runtime keeps pure code lightweight while still giving effectful work a clear lifecycle.

## Current implementation note

The shared runtime already tracks deferred and ordinary effectful work with explicit task state and memoized outcomes.

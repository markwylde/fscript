---
title: Defer and Laziness
description: Why FScript is eager by default, what defer changes, and when delayed effect start is the right tool.
---

FScript is eager by default for effectful work. `defer` is the explicit way to delay that start.

```fscript
lazyConfig = defer FileSystem.readFile('./config.json')
```

That means:

- `FileSystem.readFile(path)` starts when execution reaches it
- `defer FileSystem.readFile(path)` captures the work without starting it yet

This page is the short guide. For the full explanation, examples, and design rationale, see [Detailed Defer and Laziness](./effects/defer-and-laziness.md).

## Why `defer` exists

FScript separates two ideas:

- starting effectful work
- consuming the result of effectful work

By default, effectful calls start eagerly. `defer` exists for the cases where that is not what you want.

Use `defer` when:

- work is optional
- work is expensive
- work should begin only if a later branch needs it
- you want laziness to be clearly visible in the source

## Runtime behavior

Draft 0.1 prefers memoized single-start semantics:

- creating a deferred value captures the expression and its environment
- forcing it starts the work
- repeated force observes the same eventual result

## Related Pages

- [Effects](/fscript/language-guide/effects/)
- [Detailed Defer and Laziness](/fscript/language-guide/effects/defer-and-laziness/)
- [Execution Model](/fscript/language-guide/runtime/execution-model/)

---
title: Execution Model
description: Pure evaluation, eager effect start, implicit suspension, and explicit defer.
---

# Execution Model

FScript programs are expression-oriented and async by semantics.

## Core Rules

- pure expressions evaluate immediately
- effectful calls start eagerly when reached
- values from effectful calls suspend implicitly when consumed
- effects preserve observable ordering unless proven independent
- `defer expr` delays effect start intentionally

## Example

```fs
something = (): String => {
  filepath = '/tmp/test.txt'
  content = getContent()
  FileSystem.writeFile(filepath, content)
  content
}
```

At a high level, the runtime:

1. binds `filepath`
2. starts `getContent()` eagerly
3. suspends if `content` is needed before it is ready
4. starts `writeFile` when its dependencies are ready
5. resolves the final block value

## Why This Matters

This model lets source code stay smaller than promise-heavy JavaScript while still representing real effectful execution.

## Related Pages

- [Effects](../language-guide/effects.md)
- [Defer and laziness](../language-guide/defer-and-laziness.md)
- [Tasks](./tasks.md)

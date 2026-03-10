---
title: Execution Model
description: Pure evaluation, eager effect start, implicit suspension, and explicit defer.
---

# Execution Model

FScript programs are expression-oriented and async-by-semantics.

## The short version

- pure expressions evaluate immediately
- effectful calls start eagerly when reached
- execution suspends only when a not-yet-ready value is consumed
- `defer` delays effect start explicitly

## Example

```fscript
save = (path: String, content: String): String => {
  FileSystem.writeFile(path, content)
  content
}
```

The runtime starts effectful work as execution reaches it while preserving observable ordering rules.

## Why this is unusual

JavaScript exposes most async workflows through `Promise`, `async`, and `await`. FScript moves that concern into the runtime model instead. The goal is direct-looking code with explicit effect semantics.

## Design intent

The runtime should preserve observable source ordering unless effects are proven independent.

---
title: Control Flow
description: Use if expressions, match expressions, and try/catch as value-producing control flow constructs.
---

Control flow in FScript is expression-oriented.

## `if`

```fscript
label = if (active) { 'enabled' } else { 'disabled' }
```

Both branches produce values, and the whole `if` expression produces a value too.

## `match`

```fscript
message = match (result) {
  { tag: 'ok', value } => 'value: ' + value
  { tag: 'error', error } => 'error: ' + error.message
}
```

`match` is the preferred tool for tagged unions and shape-based branching.

## `try/catch`

```fscript
safeRead = (path: String) => {
  try {
    FileSystem.readFile(path)
  } catch (error) {
    'fallback'
  }
}
```

Like other forms, `try/catch` is value-producing.

## Comparison to JavaScript

JavaScript has these constructs too, but FScript leans much harder on them as expressions rather than statements. That makes them fit naturally inside pipelines, bindings, and helper functions.

---
title: Destructuring
description: Use record and array patterns in bindings, parameters, and match arms.
---

Patterns are used in bindings, parameters, and `match` arms.

## Record destructuring

```fscript
{ name, active } = user
```

You can also match nested structure:

```fscript
{ tag: 'member', name } = value
```

## Array destructuring

```fscript
[first, second] = items
```

## Why patterns matter

Patterns make FScript's expression-oriented style more concise:

- bindings can unpack the shape you need immediately
- `match` arms can describe both branching and extraction
- tagged unions become especially readable

## Comparison to JavaScript

The syntax is familiar if you know JS destructuring, but in FScript patterns are also a central part of matching and type narrowing rather than just a convenience for assignment.

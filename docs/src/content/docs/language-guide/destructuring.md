---
title: Destructuring
description: Use record and array patterns in bindings, parameters, and match arms.
---

Patterns are used in bindings, parameters, and `match` arms.

## Record destructuring

```fs
{ name } = user
```

```fs
{ tag: 'member', name } = account
```

## Array destructuring

```fs
[first, second] = items
```

## Why it matters

Destructuring works naturally with tagged unions and pattern matching, so it is a core part of the language rather than a minor convenience.


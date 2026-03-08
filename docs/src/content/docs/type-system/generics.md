---
title: Generics
description: Reusable type parameters for arrays, results, sequences, and other common abstractions.
---

Generics let you write reusable abstractions without giving up type precision.

```fs
type Maybe<T> = T | Null
```

You will see generics heavily in the standard library:

```fs
Array.map = <T, U>(fn: (value: T): U, items: T[]): U[]
```


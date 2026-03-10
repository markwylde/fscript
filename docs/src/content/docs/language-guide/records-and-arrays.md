---
title: Records and Arrays
description: Immutable plain data structures, structural typing, and common patterns for building new values.
---

Records and arrays are the main data structures in FScript.

## Records

```fscript
user = {
  id: '1',
  name: 'Ada',
  active: true,
}
```

Records are:

- plain data
- immutable
- structurally typed

## Arrays

```fscript
numbers = [1, 2, 3]
```

Arrays are:

- ordered
- homogeneous by default at the type level
- immutable

## Updating values

Because records and arrays are immutable, updates create new values rather than changing old ones.

```fscript
import Array from 'std:array'
import Object from 'std:object'

nextUser = Object.spread(user, { active: false })
nextNumbers = Array.append(4, numbers)
```

## Comparison to JavaScript

FScript uses record and array literals that look familiar, but it does not support mutating methods or property writes. Think "plain data plus helpers," not "objects with behavior."

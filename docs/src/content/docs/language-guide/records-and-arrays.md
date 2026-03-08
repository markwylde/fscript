---
title: Records and Arrays
description: Immutable plain data structures, structural typing, and common patterns for building new values.
---

Records and arrays are the main data structures in FScript.

```fs
user = { id: '1', name: 'Ada' }
values = [1, 2, 3]
```

## Records

- plain immutable data
- structurally typed
- field order does not matter in types

## Arrays

- homogeneous by default
- immutable
- transformed through `std:array`

## Updates

Build updated values instead of mutating in place:

```fs
import Array from 'std:array'
import Object from 'std:object'

more = Array.append(4, values)
nextUser = Object.spread(user, { active: true })
```


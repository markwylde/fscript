---
title: Array Types
description: Homogeneous immutable collections and how unions appear when array element types differ.
---

Arrays are ordered immutable collections.

```fs
numbers = [1, 2, 3] // Number[]
values = [1, 'two'] // (Number | String)[]
```

## Rules

- array element types are inferred from elements
- mixed arrays require a union element type
- index assignment is invalid
- array transforms return new arrays


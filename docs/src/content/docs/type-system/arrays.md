---
title: Array Types
description: Homogeneous immutable collections and how unions appear when array element types differ.
---

Arrays are ordered immutable collections.

## Example

```fscript
numbers = [1, 2, 3] // Number[]
```

## Rules

- element types are inferred from the contents
- mixed arrays produce union element types
- arrays are immutable
- array-transforming operations return new arrays

Example:

```fscript
values = [1, 'two'] // (Number | String)[]
```

## Comparison to JavaScript

JavaScript arrays are flexible and mutable. FScript arrays are more predictable and friendlier to static reasoning.

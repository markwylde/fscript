---
title: Data Pipelines
description: Compose immutable transformations with arrays, pipes, and curried functions.
---

# Data Pipelines

Pipes and data-last stdlib functions make collection work straightforward.

```fs
import Array from 'std:array'

numbers = [1, 2, 3, 4, 5]

result = numbers
  |> Array.map((value: Number): Number => value + 1)
  |> Array.filter((value: Number): Boolean => value > 3)
```

This style works well because:

- arrays are immutable
- transformations return new values
- standard-library functions are designed for partial application

## Related Pages

- [Pipes](../language-guide/pipes.md)
- [std:array](../standard-library/array.md)

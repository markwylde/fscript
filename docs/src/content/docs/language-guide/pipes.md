---
title: Pipes
description: Compose transformations left to right with the pipe operator and data-last helper functions.
---

The pipe operator keeps transformation code readable by letting values flow left to right.

```fs
import Array from 'std:array'

result = [1, 2, 3]
  |> Array.map((i) => i + 1)
  |> Array.filter((i) => i > 2)
```

Pipes work especially well because many FScript helpers are curried and data-last.


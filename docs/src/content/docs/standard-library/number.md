---
title: std:number
description: Numeric helpers and string-to-number parsing.
---

# `std:number`

`std:number` provides numeric helpers and basic parsing.

```fscript
import Number from 'std:number'
```

## Representative API

```fscript
Number.parse = (value: String): Number
Number.toString = (value: Number): String
Number.floor = (value: Number): Number
Number.ceil = (value: Number): Number
Number.round = (value: Number): Number
Number.min = (right: Number, left: Number): Number
Number.max = (right: Number, left: Number): Number
Number.clamp = (min: Number, max: Number, value: Number): Number
```

## Example

```fscript
port = Number.parse('3000')
safe = Number.clamp(1, 65535, port)
```

## Note

The spec leaves room for more `Result`-returning numeric helpers in future additions where failure should be modeled explicitly.

---
title: Parsing and Validation
description: Parse raw input, validate it explicitly, and return typed results.
---

# Parsing and Validation

A common FScript pattern is:

1. accept raw external input
2. inspect or parse it explicitly
3. return a typed `Result`

```fs
import Number from 'std:number'
import Result from 'std:result'
import String from 'std:string'

type ParseError = {
  tag: 'parse_error',
  message: String,
}

parsePort = (text: String): Result<Number, ParseError> => {
  if (String.isDigits(text)) {
    Result.ok(Number.parse(text))
  } else {
    Result.error({
      tag: 'parse_error',
      message: 'port must contain digits only',
    })
  }
}
```

## Related Pages

- [std:result](../standard-library/result.md)
- [std:number](../standard-library/number.md)
- [Errors](../language-guide/errors.md)

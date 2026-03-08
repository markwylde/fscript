---
title: std:logger
description: Runtime-backed terminal logging for text messages and pretty JSON output.
---

# `std:logger`

`std:logger` provides explicit terminal logging through the runtime.

```fs
import Logger from 'std:logger'
```

## Representative API

```fs
Logger.create = (options: {
  name: String | Null,
  level: 'debug' | 'info' | 'warn' | 'error',
  destination: 'stdout' | 'stderr',
}): Logger
Logger.log = (logger: Logger, message: String): Undefined
Logger.debug = (logger: Logger, message: String): Undefined
Logger.info = (logger: Logger, message: String): Undefined
Logger.warn = (logger: Logger, message: String): Undefined
Logger.error = (logger: Logger, message: String): Undefined
Logger.prettyJson = (logger: Logger, value: Unknown): Undefined
```

## Semantics

- logger operations are effectful
- output goes to the terminal selected by `destination`
- `Logger.prettyJson` should behave like logging `Json.jsonToPrettyString(value)`
- treat the value returned by `Logger.create` as the logger you pass to later logger calls

## Example

```fs
import Json from 'std:json'
import Logger from 'std:logger'

logger = Logger.create({
  name: 'config',
  level: 'info',
  destination: 'stdout',
})

config = Json.jsonToObject('{ "port": 8080, "debug": true }')
printed = Logger.prettyJson(logger, config)
```

## Pretty JSON in the Terminal

Use `Logger.prettyJson` when you want the most direct path from a value to readable terminal output.

Use `Json.jsonToPrettyString` when you want the formatted JSON as a string first, for example before writing it to a file or combining it with other text.

## Related Pages

- [std:json](./json.md)
- [Runtime boundaries](../runtime/errors-and-boundaries.md)

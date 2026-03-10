---
title: Filesystem Scripts
description: Work with runtime-backed filesystem capabilities through explicit std modules.
---

# Filesystem Scripts

Filesystem work is one of the clearest examples of FScript's effect model. Host IO is explicit and comes from `std:filesystem`.

## Example: read, trim, and log

```fscript
import FileSystem from 'std:filesystem'
import Logger from 'std:logger'
import String from 'std:string'

showConfig = (path: String): Undefined => {
  text = FileSystem.readFile(path)
  Logger.info(String.trim(text))
}

showConfig('./config.json')
```

## What this demonstrates

- no global `fs` object
- no `async` / `await`
- no `Promise` chaining in source code
- effectful work still looks sequential

## Good structure for scripts

Keep effectful boundaries thin:

```fscript
import FileSystem from 'std:filesystem'
import Json from 'std:json'
import Result from 'std:result'

readJsonFile = (path: String) => {
  text = FileSystem.readFile(path)
  Json.parse(text)
}
```

Then pass parsed data into pure helpers that handle validation and transformation.

## Current implementation note

The current runtime already ships a runtime-backed filesystem module. As with the rest of Draft 0.1, the docs aim to show the model clearly while leaving room for the API surface to keep maturing.

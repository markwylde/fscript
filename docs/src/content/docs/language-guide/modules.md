---
title: Modules
description: "Import and export rules for user modules and runtime-backed std: modules."
---

Each `.fs` file is a module. FScript supports `import` and `export`, but not CommonJS.

## Imports

```fs
import Array from 'std:array'
import { parseUser } from './user.fs'
```

## Exports

```fs
export readUser = (path: String): User => {
  text = FileSystem.readFile(path)
  parseUser(text)
}
```

## Rules

- `require` is not supported.
- Named exports are preferred for user modules.
- Default exports are especially appropriate for `std:` modules.
- Top-level module code executes once.
- Circular imports are a compile error in Draft 0.1.


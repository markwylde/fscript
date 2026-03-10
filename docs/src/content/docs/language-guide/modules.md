---
title: Modules
description: "Import and export rules for user modules and runtime-backed std: modules."
---

Each `.fs` file is a module. FScript supports `import` and `export`, but not CommonJS.

## Imports

FScript supports default imports and named imports.

```fscript
import Array from 'std:array'
import FileSystem from 'std:filesystem'
import { parseUser } from './user.fs'
```

Use default imports for `std:` modules and named exports for user modules where practical.

## Exports

You can export values and types:

```fscript
export type User = {
  id: String,
  name: String,
}

export parseUser = (text: String): User => {
  { id: text, name: text }
}
```

## Rules worth remembering

- every `.fs` file is a module
- `require` is not supported
- `module.exports` is not supported
- top-level code executes once when the module loads
- circular imports are a compile error in Draft 0.1

## Comparison to JavaScript

The surface will feel closest to ES modules, not CommonJS. The deeper difference is that module resolution and initialization semantics are defined by FScript rather than borrowed directly from Node.js.

## Good style

- prefer named exports for application modules
- keep module boundaries explicit
- separate pure helpers from effectful boundary modules when possible

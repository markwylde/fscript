---
title: Project Layout
description: How FScript source files, modules, examples, and specs are organized in the current repository.
---

In FScript, each `.fs` file is a module.

## Example repository layout

```text
examples/
specs/
crates/
docs/
```

## Module rules

- User modules use explicit relative imports.
- User module imports include the `.fs` extension.
- `std:` modules are reserved and come from the runtime.

## Example

```fs
import { parseUser } from './user.fs'
import FileSystem from 'std:filesystem'
```


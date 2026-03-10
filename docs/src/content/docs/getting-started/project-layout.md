---
title: Project Layout
description: How FScript source files, modules, examples, and specs are organized in the current repository.
---

In FScript, each `.fs` file is a module. The current repository also includes specs, examples, and Rust crates that implement the toolchain.

## Typical source layout

```text
my-app/
  src/
    main.fs
    user.fs
    config.fs
```

Example modules:

```fscript
// src/user.fs
export type User = {
  id: String,
  name: String,
}

export parseUser = (text: String): User => {
  // placeholder example
  { id: text, name: text }
}
```

```fscript
// src/main.fs
import { parseUser } from './user.fs'

main = (): String => {
  user = parseUser('ada')
  user.name
}
```

## Rules to keep in mind

- every `.fs` file is a module
- named exports are preferred for user code
- `std:` modules are imported explicitly
- CommonJS is not supported
- circular imports are a compile error in Draft 0.1

## Repo-level layout

The repository itself is organized roughly like this:

```text
fscript/
  crates/
  docs/
  examples/
  specs/
```

What each area is for:

- `crates/`: Rust implementation of the CLI, parser, typechecker, runtime, interpreter, and codegen
- `examples/`: small FScript programs used for testing and demonstration
- `specs/`: language, runtime, type system, and implementation design docs
- `docs/`: user-facing handbook and reference site content

## Comparison to TypeScript projects

If you are used to a TypeScript app layout, the biggest differences are:

- no `package.json` is required by the language itself
- no `node_modules` assumption is built into module loading
- source modules are plain `.fs` files
- standard-library access uses `std:` imports instead of global runtime objects

## Good habits early on

- keep modules small and explicit
- export types and functions directly from the files that own them
- separate parsing, validation, and IO concerns into different modules when practical
- treat records and arrays as immutable values from the start

## Next step

Read [Running, Checking, and Compiling](./running-checking-compiling.md) to see how the CLI maps onto that project layout.
